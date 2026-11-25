//! The order follows its payment: state, settlement summary, events and the release of reserved stock in one transaction.
use crate::domain::attempt::Attempt;
use crate::domain::{settlements, state};
use crate::ports::Host;
use crate::store::attempts::attempt;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Result, conflict};
use veyndo_orders::domain::order_fields::order_fields;
use veyndo_pricing::verified_kernel;

pub async fn update_order(
    host: &dyn Host,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
) -> Result<()> {
    let mut o: Value =
        sqlx::query_scalar("SELECT data FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(&p.tenant)
            .bind(&p.order)
            .fetch_one(&mut **tx)
            .await?;
    o["payment"]["state"] = json!(state);
    let current = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2")
        .bind(&p.tenant)
        .bind(&p.id)
        .fetch_one(&mut **tx)
        .await?;
    let summary = settlements::summary(&attempt(&current));
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
    o["revision"] = json!(o["revision"].as_i64().unwrap_or(1) + 1);
    if state == "captured_late" {
        o["state"] = json!("payment_review");
    }
    if ["cancelled", "expired"].contains(&state) {
        o["state"] = json!(state);
    }
    host.captured(tx, &p.tenant, &mut o, state).await?;
    order_fields(&mut o);
    sqlx::query("UPDATE orders SET data=$1 WHERE tenant=$2 AND id=$3")
        .bind(&o)
        .bind(&p.tenant)
        .bind(&p.order)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE carts SET data=jsonb_set(data,'{order}',$1) WHERE tenant=$2 AND id=$3")
        .bind(&o)
        .bind(&p.tenant)
        .bind(o["cart"]["id"].as_str().unwrap())
        .execute(&mut **tx)
        .await?;
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
    if state == "captured" && let Some(e)=sqlx::query("UPDATE exposures SET rewarded=true WHERE tenant=$1 AND session=(SELECT data->>'session' FROM carts WHERE tenant=$1 AND id=$2) AND rewarded=false RETURNING variant").bind(&p.tenant).bind(o["cart"]["id"].as_str().unwrap()).fetch_optional(&mut **tx).await?{sqlx::query("UPDATE policy SET purchases=purchases+1 WHERE tenant=$1 AND variant=$2").bind(&p.tenant).bind(e.get::<String,_>("variant")).execute(&mut **tx).await?;}
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
    sqlx::query("UPDATE inventory_reservations SET released=true WHERE tenant=$1 AND attempt_id=$2 AND NOT released")
        .bind(&p.tenant).bind(&p.id).execute(&mut **tx).await?;
    sqlx::query(
        "UPDATE payment_attempts SET state=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
    )
    .bind(state)
    .bind(&p.tenant)
    .bind(&p.id)
    .execute(&mut **tx)
    .await?;
    update_order(host, tx, p, state).await
}
