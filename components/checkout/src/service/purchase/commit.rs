//! Final admission and one atomic order, stock, cart, payment-attempt and outbox commit; no external provider call.
use super::{Locked, Placed, Priced};
use crate::domain::admin_order::{admit_payment, order as admin_order};
use crate::domain::review_binding::validate as validate_review;
use crate::domain::session_review::Review;
use crate::ports::Purchase;
use crate::store::customer_snapshot::snapshot as customer_snapshot;
use crate::store::session::admit_channel;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_b2b::service::quote_contract::consume as consume_quote;
use veyndo_b2b::store::purchase::{check_locked, record};
use veyndo_catalog::store::order_downloads::grant;
use veyndo_core::shop_scope::{header, live_parent};
use veyndo_core::{RequestContext, Result, bad, process_config, uid};
use veyndo_inventory::InventoryLines;
use veyndo_inventory::store::reservations::{allocate, consume};
use veyndo_marketing::store::promotions::record_uses;
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_payments::service::funding::prepare as prepare_funding;
use veyndo_payments::service::sessions::origin_conn;
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::StoredCart;

/// Prepares the provider payment of an external order and stores the order with it.
async fn prepare_external<P: Purchase>(
    port: &P,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
    (q, order): (&Value, &mut Value),
    minor: i64,
    review: Option<&Review>,
) -> Result<()> {
    let return_origin = match header(h, "origin") {
        Some(origin) if q["paymentMethod"]["provider"].is_string() => {
            Some(origin_conn(tx, &c.tenant, &c.data.sales_channel, Some(origin)).await?)
        }
        _ => None,
    };
    port.prepare_payment(tx, c, order, minor, return_origin.as_deref())
        .await?;
    if let Some(review) = review {
        review.check_prepared_payment(&order["payment"], process_config::get().strict)?;
    }
    prepare_funding(tx, h, c, order).await?;
    order_fields(order);
    sqlx::query("UPDATE orders SET data=$1 WHERE id=$2")
        .bind(&*order)
        .bind(order["id"].as_str())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The review binding and the tenant policy of a business cart.
async fn admit<P: Purchase>(
    port: &P,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    h: &RequestContext,
    locked: (&StoredCart, &P::Prepared),
    priced: &Priced,
    review: Option<&Review>,
) -> Result<()> {
    let (c, modules) = locked;
    let (q, minor) = (&priced.q, priced.minor);
    validate_review(h, c, q, minor)?;
    if let Some(review) = review {
        admit_channel(tx, c).await?;
        review.check_quote(q, &priced.legal_snapshot["policyVersion"], minor)?;
    }
    if priced.config.is_business(&c.data.group) {
        let digest: String =
            sqlx::query_scalar("SELECT digest FROM extensions WHERE tenant=$1 FOR SHARE")
                .bind(&c.tenant)
                .fetch_one(&mut **tx)
                .await?;
        port.approve(modules, &digest, minor)?;
    }
    Ok(())
}

/// The payment admission after the company budget: merchant payment, sandbox, provider, settlement and contact data.
async fn admit_payment_contact<P: Purchase>(
    port: &P,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    h: &RequestContext,
    c: &StoredCart,
    priced: &Priced,
) -> Result<bool> {
    let (q, minor) = (&priced.q, priced.minor);
    admit_payment(c, q)?;
    let external = q["paymentMethod"]["mode"] == "app";
    if external && live_parent(tx, &c.tenant).await?.is_some() {
        return Err(bad("External payments are disabled in private sandboxes"));
    }
    if external && q["paymentMethod"]["provider"].is_null() {
        port.provider_ready(&c.tenant, &priced.selected.payment_method_id)?;
    }
    // Financial providers/manual invoices require usable contact and billing data.
    // The explicit simulated demo method keeps legacy headless fixture carts compatible.
    port.settlement(h, c, q, &priced.config, minor)?;
    if !verified_kernel::checkout_contact_admissible(
        q["paymentMethod"]["mode"] == "simulated"
            || (q["paymentMethod"]["mode"] == "cash" && h.pos.is_some()),
        c.data.email.is_some(),
        priced.selected.billing_address.is_some(),
    ) {
        return Err(bad("Customer email and billing address required"));
    }
    Ok(external)
}

/// The order document before its lines touch stock: quote, payment state, lifecycle, money, legal, company and customer.
fn order_document(
    c: &StoredCart,
    priced: &Priced,
    id: &str,
    locale: Option<&String>,
    review: Option<&Review>,
) -> Value {
    let q = &priced.q;
    let mut order = json!({"id":id,"orderNumber":format!("RAC-{}",&id[..8]),"cart":q,"state":"placed","channel":c.data.channel,"revision":1,"deliveries":q["deliveries"],"payment":{"method":q["paymentMethod"],"provider":if q["paymentMethod"]["mode"]=="simulated"{"simulated"}else{"manual"},"state":if q["paymentMethod"]["mode"]=="simulated"{"authorized"}else{"pending"},"realMoneyCharged":false},"customerGroup":c.data.group});
    if let Some(review) = review {
        order["checkoutSessionId"] = json!(review.session());
    }
    order["cart"]["locale"] = json!(locale);
    order
}

/// Commits the order with its stock, company budget, promotion uses, downloads, payment and outbox event.
pub(super) async fn run<P: Purchase>(
    port: &P,
    h: &RequestContext,
    key: &str,
    locked: Locked<P::Prepared>,
    mut priced: Priced,
    review: Option<&Review>,
) -> Result<Placed> {
    let Locked {
        mut tx,
        cart: mut c,
        prepared,
        fingerprint,
    } = locked;
    admit(port, &mut tx, h, (&c, &prepared.modules), &priced, review).await?;
    let minor = priced.minor;
    priced.q["price"]["totalMinor"] = json!(minor.to_string());
    let procurement = check_locked(
        &mut tx,
        h,
        &c,
        &priced.q,
        u64::try_from(minor).map_err(|_| bad("Invalid company total"))?,
    )
    .await?;
    let external = admit_payment_contact(port, &mut tx, h, &c, &priced).await?;
    let customer = customer_snapshot(&mut tx, &c, &priced.selected).await?;
    let id = uid();
    // Explicit simulated authorization: no external money is charged.
    let mut order = order_document(&c, &priced, &id, prepared.locale_chain.first(), review);
    port.bind_order(&mut tx, &c.tenant, &mut order).await?;
    admin_order(&c, &mut order);
    order["money"] = json!(priced.money);
    order["currencyContext"] = priced.q["currencyContext"].clone();
    order["legal"] = priced.legal_snapshot.clone();
    if let Some(p) = &procurement {
        order["b2b"] = json!({"companyId":p.company_id,"unitId":p.context.unit_id,"purchaseOrder":p.context.purchase_order,"approvalId":p.approval,"quoteId":p.context.quote_id});
    }
    if let (Some(order), Some(customer)) = (order.as_object_mut(), customer.as_object()) {
        order.extend(customer.clone());
    }
    port.settle(&mut tx, h, &c, &mut order).await?;
    order_fields(&mut order);
    sqlx::query("INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data) VALUES($1,$2,$3,$4,$5,$6)").bind(&id).bind(&c.tenant).bind(&c.id).bind(key).bind(&fingerprint).bind(&order).execute(&mut *tx).await?;
    record(&mut tx, &c, &id, procurement.as_ref()).await?;
    consume_quote(&mut tx, &c, &id).await?;
    let inventory = InventoryLines::new(c.data.items.iter().map(|i| (i.id.as_str(), i.quantity)))?;
    allocate(&mut tx, &c.tenant, &inventory, &id).await?;
    record_uses(&mut tx, &c.tenant, &id, &priced.q).await?;
    let products: Vec<String> = c.data.items.iter().map(|i| i.id.clone()).collect();
    grant(&mut tx, &c.tenant, &products, &id).await?;
    if external {
        prepare_external(port, &mut tx, h, &c, (&priced.q, &mut order), minor, review).await?;
    }
    consume(&mut tx, &c.tenant, &inventory).await?;
    c.data.order = Some(order.clone());
    sqlx::query("UPDATE carts SET status='completed',data=$1,revision=revision+1 WHERE id=$2")
        .bind(json!(c.data))
        .bind(&c.id)
        .execute(&mut *tx)
        .await?;
    if !external && let Some(e)=sqlx::query("UPDATE exposures SET rewarded=true WHERE tenant=$1 AND session=$2 AND rewarded=false RETURNING variant").bind(&c.tenant).bind(&c.data.session).fetch_optional(&mut *tx).await? {
        sqlx::query("UPDATE policy SET purchases=purchases+1 WHERE tenant=$1 AND variant=$2").bind(&c.tenant).bind(e.get::<String,_>("variant")).execute(&mut *tx).await?;
    }
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &c.tenant,
        "order.placed",
        &json!({"orderId":id,"totalMinor":minor}),
    )
    .await?;
    port.fence(&mut tx).await?;
    tx.commit().await?;
    Ok(Placed {
        order,
        cart: c,
        method: priced.selected.payment_method_id,
    })
}
