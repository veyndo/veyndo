//! The outbox event that makes accepted recipient instructions available, with a native invoice snapshot.
use crate::domain::attempt::Attempt;
use crate::domain::instructions::snapshot;
use serde_json::{Value, json};
use veyndo_core::Result;

pub async fn publish(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    context: &Value,
) -> Result<()> {
    let mut order: Value = sqlx::query_scalar("SELECT data FROM orders WHERE tenant=$1 AND id=$2")
        .bind(&p.tenant)
        .bind(&p.order)
        .fetch_one(&mut **tx)
        .await?;
    if let Some(cart) = order["cart"].as_object_mut() {
        cart.remove("token");
    }
    let data = json!({"orderId":p.order,"attemptId":p.id,"order":order,"paymentInstructions":snapshot(p,&context["recipientInstructions"])?});
    sqlx::query(
        "INSERT INTO outbox(tenant,kind,data) VALUES($1,'payment.instructions.available',$2)",
    )
    .bind(&p.tenant)
    .bind(data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
