//! The order follows its payment: state, settlement summary, events and the release of reserved stock in one transaction.
use crate::domain::attempt::Attempt;
use crate::domain::{settlements, state};
use crate::ports::Host;
use crate::store::attempts;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, conflict};
use veyndo_inventory::store::attempt_reservations::release_attempt;
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_pricing::verified_kernel;

/// The order document of a payment (a missing row is a database error).
pub async fn order_document<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    tenant: &str,
    order: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2",
        tenant,
        order
    )
    .fetch_one(db)
    .await?)
}

/// The order fields the ledger reads back while updating the payment state.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LedgerOrder {
    revision: Value,
    #[serde(deserialize_with = "lenient")]
    cart: LedgerCart,
}

/// The cart an order was placed from.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LedgerCart {
    id: Value,
}

pub async fn update_order(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
) -> Result<()> {
    let mut o: Value = sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE",
        p.tenant,
        p.order
    )
    .fetch_one(&mut **tx)
    .await?;
    o["payment"]["state"] = json!(state);
    let current = attempts::get(&mut **tx, &p.tenant, &p.id).await?;
    let summary = settlements::summary(&current);
    if let (Some(payment), Some(fields)) = (o["payment"].as_object_mut(), summary.as_object()) {
        payment.extend(fields.clone());
    }
    o["payment"]["realMoneyCharged"] = json!(
        p.environment == "live"
            && [
                "captured",
                "partially_captured",
                "captured_late",
                "partially_refunded",
                "refunded"
            ]
            .contains(&state)
    );
    o["revision"] = json!(view::<LedgerOrder>(&o).revision.as_i64().unwrap_or(1) + 1);
    if state == "captured_late" {
        o["state"] = json!("payment_review");
    }
    if ["cancelled", "expired"].contains(&state) {
        o["state"] = json!(state);
    }
    host.captured(tx, &p.tenant, &mut o, state).await?;
    order_fields(&mut o);
    let cart = view::<LedgerOrder>(&o).cart.id;
    let cart = cart.as_str().unwrap();
    sqlx::query!(
        "UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3",
        o,
        p.tenant,
        p.order
    )
    .execute(&mut **tx)
    .await?;
    host.mirror_cart_order(tx, &p.tenant, cart, &o).await?;
    let mut event_order = o.clone();
    if let Some(cart) = event_order["cart"].as_object_mut() {
        cart.remove("token");
    }
    let kind = if state == "captured" && p.state != "captured" {
        "payment.captured"
    } else {
        "payment.updated"
    };
    veyndo_core::store::outbox::publish_on(&mut **tx, &p.tenant, kind, &json!({"orderId":p.order,"attemptId":p.id,"state":state,"provider":p.provider,"environment":p.environment,"realMoneyCharged":p.environment=="live" && state=="captured","order":event_order})).await?;
    if state == "captured" {
        host.reward_capture(tx, &p.tenant, cart).await?;
    }
    Ok(())
}
pub async fn release_stock(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
    void_confirmed: bool,
) -> Result<()> {
    if !verified_kernel::reservation_release_admissible(
        ["pending", "ready", "approved"].contains(&p.state.as_str()),
        p.state == "authorized",
        void_confirmed,
    ) {
        return Err(conflict(
            "Captured or uncertain payment cannot release inventory",
        ));
    }
    state::transition(&p.state, state)?;
    veyndo_inventory::store::reservations::release(tx, &p.tenant, &p.order).await?;
    release_attempt(tx, &p.tenant, &p.id).await?;
    sqlx::query!(
        "UPDATE payment_attempts SET state=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
        state,
        p.tenant,
        p.id
    )
    .execute(&mut **tx)
    .await?;
    update_order(host, tx, p, state).await
}
