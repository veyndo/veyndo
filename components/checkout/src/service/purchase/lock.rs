//! Ordered lock and replay admission: idempotency key, merchant channel policy, cart, company unit, reviewed session and the purchase fingerprint.
use super::{Admission, Locked, Prepared};
use crate::domain::session_review::Review;
use crate::store::admin_order::lock_checkout;
use crate::store::carts::lock_by_token;
use crate::store::purchase::lock_key as lock_idempotency_key;
use crate::store::session::admit_channel;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_b2b::store::access::{guard_conn, identity};
use veyndo_b2b::store::purchase::lock_unit;
use veyndo_core::request_context::PosAdmission;
use veyndo_core::shop_scope::{channel, tenant, token};
use veyndo_core::store::history_context::{context, customer_context};
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash, process_config};
use veyndo_marketing::service::conditions::validate_condition;
use veyndo_orders::store::placement::placed;
use veyndo_pricing::verified_kernel;
use veyndo_shop::domain::cart_admission::request;
use veyndo_shop::types::cart::{AppConfiguration, BusinessContext, Discount, Item, StoredCart};
use veyndo_shop::types::settings::CheckoutSelection;

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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PurchaseDocument<'a> {
    items: &'a Vec<Item>,
    checkout: &'a Option<CheckoutSelection>,
    group: &'a str,
    buyer: &'a Option<Value>,
    coupons: &'a Vec<String>,
    sales_channel: &'a str,
    currency: &'a str,
    b2b: &'a Option<BusinessContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_configurations: Option<&'a HashMap<String, AppConfiguration>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pos_discount: Option<&'a Discount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pos_customer: Option<&'a str>,
    /// Present for a cashier purchase, null without a tender.
    #[serde(skip_serializing_if = "Option::is_none")]
    pos_tender: Option<Option<u64>>,
    /// The merchant who placed the purchase and the customer they selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_actor: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    admin_customer: Option<Option<&'a str>>,
}

/// The purchase document the fingerprint hashes; a cashier purchase adds its discount, customer and tender.
fn purchase_document(c: &StoredCart, pos: Option<&PosAdmission>) -> Value {
    let admin = c.data.admin_actor.as_deref();
    json!(PurchaseDocument {
        items: &c.data.items,
        checkout: &c.data.checkout,
        group: &c.data.group,
        buyer: &c.data.buyer,
        coupons: &c.data.coupons,
        sales_channel: &c.data.sales_channel,
        currency: &c.data.currency,
        b2b: &c.data.b2b,
        app_configurations: Some(&c.data.app_configurations).filter(|v| !v.is_empty()),
        pos_discount: pos.and(c.data.pos_discount.as_ref()),
        pos_customer: pos.and(c.data.customer_id.as_deref()),
        pos_tender: pos.map(|p| p.tender_minor),
        admin_actor: admin,
        admin_customer: admin.map(|_| c.data.customer_id.as_deref()),
    })
}

/// The fingerprint of what the order commits to.
fn purchase_fingerprint(h: &RequestContext, c: &StoredCart) -> String {
    hash(&format!(
        "{}:{}",
        c.id,
        purchase_document(c, h.pos.as_ref())
    ))
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

#[cfg(test)]
mod tests {
    use super::purchase_document;
    use serde_json::{Value, json};
    use veyndo_core::request_context::PosAdmission;
    use veyndo_shop::types::cart::StoredCart;

    /// The previous indexed construction, kept as the oracle of the typed document.
    fn legacy(c: &StoredCart, pos: Option<&PosAdmission>) -> Value {
        let mut purchase = json!({"items":c.data.items,"checkout":c.data.checkout,"group":c.data.group,"buyer":c.data.buyer,"coupons":c.data.coupons,"salesChannel":c.data.sales_channel,"currency":c.data.currency,"b2b":c.data.b2b});
        if !c.data.app_configurations.is_empty() {
            purchase["appConfigurations"] = json!(c.data.app_configurations);
        }
        if let Some(pos) = pos {
            if let Some(discount) = &c.data.pos_discount {
                purchase["posDiscount"] = json!(discount);
            }
            if let Some(customer) = &c.data.customer_id {
                purchase["posCustomer"] = json!(customer);
            }
            purchase["posTender"] = json!(pos.tender_minor);
        }
        if let Some(actor) = &c.data.admin_actor {
            purchase["adminActor"] = json!(actor);
            purchase["adminCustomer"] = json!(c.data.customer_id);
        }
        purchase
    }

    fn cart(extra: Value) -> StoredCart {
        let mut data = json!({"items":[{"id":"p1","quantity":2}],"group":"retail","session":"s","email":null,"company":null,"buyer":null,"order":null,"coupons":["SAVE"]});
        for (k, v) in extra.as_object().unwrap() {
            data[k] = v.clone();
        }
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(data).expect("cart"),
            revision: 1,
            status: "open".into(),
        }
    }

    /// The typed purchase document equals the indexed one, so existing fingerprints stay valid.
    #[test]
    fn the_typed_purchase_document_matches_the_indexed_one() {
        let carts = [
            cart(json!({})),
            cart(json!({"customer_id":"c1","admin_actor":"merchant"})),
            cart(json!({"admin_actor":"merchant"})),
            cart(
                json!({"customer_id":"c1","pos_discount":{"kind":"percent","value":5,"currency":"EUR","reason":"r","actor":"a"},
                "app_configurations":{"x":{"app":"a","productId":"p1","fields":{},"feeMinor":0,"ruleRevision":1,"appVersion":"1","entity":"e","record":"r"}}}),
            ),
        ];
        let tenders = [
            None,
            Some(PosAdmission {
                actor: "a".into(),
                tender_minor: None,
            }),
            Some(PosAdmission {
                actor: "a".into(),
                tender_minor: Some(500),
            }),
        ];
        for c in &carts {
            for pos in &tenders {
                let typed = purchase_document(c, pos.as_ref());
                assert_eq!(typed, legacy(c, pos.as_ref()));
                assert_eq!(typed.to_string(), legacy(c, pos.as_ref()).to_string());
            }
        }
    }
}
