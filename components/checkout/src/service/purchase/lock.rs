//! Ordered lock and replay admission: idempotency key, merchant channel policy, cart, company unit, reviewed session and the purchase fingerprint.
use super::{Admission, Locked, Prepared};
use crate::domain::admin_order::purchase as admin_purchase;
use crate::domain::session_review::Review;
use crate::store::admin_order::lock_checkout;
use crate::store::carts::lock_by_token;
use crate::store::purchase::{lock_key as lock_idempotency_key, placed};
use crate::store::session::admit_channel;
use serde_json::json;
use veyndo_b2b::store::access::{guard_conn, identity};
use veyndo_b2b::store::purchase::lock_unit;
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::store::history_context::{context, customer_context};
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash, process_config};
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_pricing::verified_kernel;
use veyndo_shop::domain::cart_admission::request;
use veyndo_shop::types::cart::StoredCart;

/// The locked cart still belongs to the reviewed session and its current buyer.
async fn admit_reviewed(
    review: &Review,
    tx: &mut sqlx::PgConnection,
    h: &RequestContext,
    c: &StoredCart,
) -> Result<()> {
    let buyer = identity(tx, h).await?;
    review.admit_cart(c, channel(h), &buyer)
}

/// What the order commits to: the lines and every choice that changes its price or recipient.
fn purchase_fingerprint(h: &RequestContext, c: &StoredCart) -> String {
    let mut purchase = json!({"items":c.data.items,"checkout":c.data.checkout,"group":c.data.group,"buyer":c.data.buyer,"coupons":c.data.coupons,"salesChannel":c.data.sales_channel,"currency":c.data.currency,"b2b":c.data.b2b});
    if !c.data.app_configurations.is_empty() {
        purchase["appConfigurations"] = json!(c.data.app_configurations);
    }
    if h.pos.is_some() {
        if let Some(discount) = &c.data.pos_discount {
            purchase["posDiscount"] = json!(discount);
        }
        if let Some(customer) = &c.data.customer_id {
            purchase["posCustomer"] = json!(customer);
        }
        purchase["posTender"] = json!(h.pos.as_ref().and_then(|p| p.tender_minor));
    }
    admin_purchase(c, &mut purchase);
    hash(&format!("{}:{}", c.id, purchase))
}

/// Takes the purchase locks in their fixed order; a repeated key with the same purchase replays the first order.
pub(super) async fn run<P>(
    a: &State,
    h: &RequestContext,
    key: &str,
    prepared: Prepared<P>,
    review: Option<&Review>,
) -> Result<Admission<Locked<P>>> {
    let mut tx = a.db.begin().await?;
    context(&mut tx, h, "checkout").await?;
    let lock_key = format!("{}:{key}", tenant(h)?);
    lock_idempotency_key(&mut tx, &lock_key).await?;
    lock_checkout(&mut tx, &prepared.admitted, &validate_condition).await?;
    let c = lock_by_token(&mut tx, &tenant(h)?, token(h)?)
        .await?
        .ok_or(bad("Cart not found"))?
        .into_cart()?;
    request(h, &c)?;
    lock_unit(&mut tx, &c).await?;
    guard_conn(&mut tx, h, &c).await?;
    if let Some(review) = review {
        admit_reviewed(review, &mut tx, h, &c).await?;
    }
    if c.data.locale != prepared.admitted.data.locale {
        return Err(conflict("Checkout language changed; review again"));
    }
    if c.data.customer_id.is_some()
        && let Some(email) = &c.data.email
    {
        customer_context(&mut tx, email).await?;
    }
    let fingerprint = purchase_fingerprint(h, &c);
    if let Some(r) = placed(&mut tx, &c.tenant, key).await? {
        if !verified_kernel::replay_admissible(
            r.cart_id == c.id,
            c.status == "open",
            r.fingerprint == fingerprint,
        ) {
            return Err(conflict("Idempotency key was used for another purchase"));
        }
        let order = r.data;
        if let Some(review) = review {
            admit_channel(&mut tx, &c).await?;
            review.check_replay(&order, process_config::get().strict)?;
        }
        return Ok(Admission::Replay(order));
    }
    if c.status != "open" {
        return Err(conflict("Cart already completed or cancelled"));
    }
    if c.data.items.is_empty() {
        return Err(bad("Cart is empty"));
    }
    Ok(Admission::Active(Locked {
        tx,
        cart: c,
        prepared,
        fingerprint,
    }))
}
