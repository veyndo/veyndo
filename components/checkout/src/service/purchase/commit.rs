//! Final admission and one atomic order, stock, cart, payment-attempt and outbox commit; no external provider call.
use super::{Locked, Placed, Priced};
use crate::domain::admin_order::{admit_payment, order as admin_order};
use crate::domain::quote_view::{OrderRef, quote_view};
use crate::domain::review_binding::validate as validate_review;
use crate::domain::session_review::Review;
use crate::ports::Purchase;
use crate::store::customer_snapshot::snapshot as customer_snapshot;
use crate::store::purchase;
use crate::store::session::admit_channel;
use crate::types::events::OrderPlaced;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_b2b::service::quote_contract::consume as consume_quote;
use veyndo_b2b::store::purchase::{Purchase as CompanyPurchase, check_locked, record};
use veyndo_catalog::store::order_downloads::grant;
use veyndo_core::document::{merge, view};
use veyndo_core::document_slot::field;
use veyndo_core::shop_scope::header;
use veyndo_core::store::environments::live_parent;
use veyndo_core::{RequestContext, Result, bad, process_config, uid};
use veyndo_inventory::InventoryLines;
use veyndo_inventory::store::reservations::{allocate, consume};
use veyndo_marketing::store::promotions::record_uses;
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_orders::store::placement::{self, NewOrder};
use veyndo_payments::service::funding::prepare as prepare_funding;
use veyndo_payments::service::sessions::origin_conn;
use veyndo_pricing::domain::money::Money;
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::StoredCart;

/// The legal policy version a purchase froze.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LegalSnapshot {
    policy_version: Value,
}

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
        Some(origin) if quote_view(q).method().provider.is_string() => {
            Some(origin_conn(tx, &c.tenant, &c.data.sales_channel, Some(origin)).await?)
        }
        _ => None,
    };
    port.prepare_payment(tx, c, order, minor, return_origin.as_deref())
        .await?;
    if let Some(review) = review {
        review.check_prepared_payment(
            &view::<OrderRef>(order).payment,
            process_config::get().strict,
        )?;
    }
    prepare_funding(port, tx, h, c, order).await?;
    order_fields(order);
    let id = view::<OrderRef>(order).id;
    placement::update_order(tx, id.as_str(), order).await?;
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
        review.check_quote(
            q,
            &view::<LegalSnapshot>(&priced.legal_snapshot).policy_version,
            minor,
        )?;
    }
    if priced.config.is_business(&c.data.group) {
        let digest = port.policy_digest(tx, &c.tenant).await?;
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
    let method = quote_view(q).method();
    let external = method.mode == "app";
    if external && live_parent(tx, &c.tenant).await?.is_some() {
        return Err(bad("External payments are disabled in private sandboxes"));
    }
    if external && method.provider.is_null() {
        port.provider_ready(&c.tenant, &priced.selected.payment_method_id)?;
    }
    // Financial providers/manual invoices require usable contact and billing data.
    // The explicit simulated demo method keeps legacy headless fixture carts compatible.
    port.settlement(h, c, q, &priced.config, minor)?;
    if !verified_kernel::checkout_contact_admissible(
        method.mode == "simulated" || (method.mode == "cash" && h.pos.is_some()),
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
) -> Result<Value> {
    let q = &priced.q;
    let quoted = quote_view(q);
    let simulated = quoted.method().mode == "simulated";
    let mut cart = q.clone();
    merge(&mut cart, CartLocale { locale })?;
    let mut order = json!({"id":id,"orderNumber":format!("RAC-{}",&id[..8]),"cart":cart,"state":"placed","channel":c.data.channel,"revision":1,"deliveries":quoted.deliveries,"payment":{"method":quoted.payment_method,"provider":if simulated{"simulated"}else{"manual"},"state":if simulated{"authorized"}else{"pending"},"realMoneyCharged":false},"customerGroup":c.data.group});
    if let Some(review) = review {
        merge(
            &mut order,
            CheckoutSession {
                checkout_session_id: review.session(),
            },
        )?;
    }
    Ok(order)
}

/// The locale the order's cart was placed in.
#[derive(Serialize)]
struct CartLocale<'a> {
    locale: Option<&'a String>,
}

/// The checkout session that placed an order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutSession<'a> {
    checkout_session_id: &'a str,
}

/// The exact minor total a purchase froze into its quote price.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrozenTotal {
    total_minor: String,
}

/// The money, currency, legal and company terms an order is placed under.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderTerms<'a> {
    money: &'a Money,
    currency_context: Value,
    legal: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    b2b: Option<OrderCompany<'a>>,
}

/// The company purchase an order belongs to.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderCompany<'a> {
    company_id: &'a str,
    unit_id: &'a str,
    purchase_order: &'a str,
    approval_id: Option<&'a str>,
    quote_id: Option<&'a str>,
}

impl<'a> OrderCompany<'a> {
    fn of(p: &'a CompanyPurchase) -> Self {
        OrderCompany {
            company_id: &p.company_id,
            unit_id: &p.context.unit_id,
            purchase_order: &p.context.purchase_order,
            approval_id: p.approval.as_deref(),
            quote_id: p.context.quote_id.as_deref(),
        }
    }
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
    merge(
        field(&mut priced.q, "price")?,
        FrozenTotal {
            total_minor: minor.to_string(),
        },
    )?;
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
    let mut order = order_document(&c, &priced, &id, prepared.locale_chain.first(), review)?;
    port.bind_order(&mut tx, &c.tenant, &mut order).await?;
    admin_order(&c, &mut order)?;
    merge(
        &mut order,
        OrderTerms {
            money: &priced.money,
            currency_context: quote_view(&priced.q).currency_context,
            legal: &priced.legal_snapshot,
            b2b: procurement.as_ref().map(OrderCompany::of),
        },
    )?;
    if let (Some(order), Some(customer)) = (order.as_object_mut(), customer.as_object()) {
        order.extend(customer.clone());
    }
    port.settle(&mut tx, h, &c, &mut order).await?;
    order_fields(&mut order);
    placement::insert_order(
        &mut tx,
        &NewOrder {
            id: &id,
            tenant: &c.tenant,
            cart: &c.id,
            key,
            fingerprint: &fingerprint,
            data: &order,
        },
    )
    .await?;
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
    purchase::complete_cart(&mut tx, &c.id, &json!(c.data)).await?;
    if !external
        && let Some(variant) =
            purchase::reward_exposure(&mut tx, &c.tenant, &c.data.session).await?
    {
        port.count_purchase(&mut tx, &c.tenant, &variant).await?;
    }
    let placed = OrderPlaced {
        order_id: &id,
        total_minor: minor,
    };
    veyndo_core::events::emit(&mut *tx, &c.tenant, &placed).await?;
    port.fence(&mut tx).await?;
    tx.commit().await?;
    Ok(Placed {
        order,
        cart: c,
        method: priced.selected.payment_method_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{CartLocale, CheckoutSession, FrozenTotal, OrderCompany, OrderTerms};
    use serde_json::json;
    use veyndo_core::document::merge;
    use veyndo_core::document_slot::field;
    use veyndo_pricing::domain::money::{Currency, Money};

    /// Each typed order patch writes exactly the document the indexed assignments wrote.
    #[test]
    fn typed_order_patches_match_the_indexed_writes() {
        let money = Money::new(1250, Currency::new("EUR", 2).unwrap());
        let legal = json!({"policyVersion":"v1","consent":{"decided":false}});
        let base =
            json!({"id":"o1","cart":{"id":"c1","price":{"totalPrice":12.5}},"state":"placed"});
        let locale = "de-DE".to_string();
        for (with_locale, review, company) in [(true, true, true), (false, false, false)] {
            let locale = with_locale.then_some(&locale);
            let mut indexed = base.clone();
            if review {
                indexed["checkoutSessionId"] = json!("session-1");
            }
            indexed["cart"]["locale"] = json!(locale);
            indexed["cart"]["price"]["totalMinor"] = json!("1250");
            indexed["money"] = json!(money);
            indexed["currencyContext"] = json!({"base":"EUR"});
            indexed["legal"] = legal.clone();
            if company {
                indexed["b2b"] = json!({"companyId":"co","unitId":"u","purchaseOrder":"PO-1","approvalId":null,"quoteId":"q"});
            }
            let mut typed = base.clone();
            if review {
                merge(
                    &mut typed,
                    CheckoutSession {
                        checkout_session_id: "session-1",
                    },
                )
                .unwrap();
            }
            merge(field(&mut typed, "cart").unwrap(), CartLocale { locale }).unwrap();
            merge(
                field(field(&mut typed, "cart").unwrap(), "price").unwrap(),
                FrozenTotal {
                    total_minor: "1250".into(),
                },
            )
            .unwrap();
            merge(
                &mut typed,
                OrderTerms {
                    money: &money,
                    currency_context: json!({"base":"EUR"}),
                    legal: &legal,
                    b2b: company.then_some(OrderCompany {
                        company_id: "co",
                        unit_id: "u",
                        purchase_order: "PO-1",
                        approval_id: None,
                        quote_id: Some("q"),
                    }),
                },
            )
            .unwrap();
            assert_eq!(typed, indexed);
        }
    }
}
