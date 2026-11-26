//! The outbox event that makes accepted recipient instructions available, with a native invoice snapshot.
use crate::domain::attempt::Attempt;
use crate::domain::instructions::snapshot;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::Result;
use veyndo_core::document::view;
use veyndo_orders::domain::order_patches::strip_cart_token;

/// The JSON fields `publish` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublishInput {
    recipient_instructions: Value,
}

pub async fn publish(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    context: &Value,
) -> Result<()> {
    let input: PublishInput = view(context);
    let mut order: Value = sqlx::query_scalar!(
        "SELECT data FROM orders WHERE tenant=$1 AND id=$2",
        p.tenant,
        p.order
    )
    .fetch_one(&mut **tx)
    .await?;
    strip_cart_token(&mut order)?;
    let data = json!({"orderId":p.order,"attemptId":p.id,"order":order,"paymentInstructions":snapshot(p,&input.recipient_instructions)?});
    veyndo_core::store::outbox::publish_on(
        &mut **tx,
        &p.tenant,
        "payment.instructions.available",
        &data,
    )
    .await?;
    Ok(())
}
