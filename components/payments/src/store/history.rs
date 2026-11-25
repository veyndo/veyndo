//! The payment-only projection of an order's native entity history; full snapshots with customer PII stay behind their own permissions. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// One finalized order change that touched the payment.
pub struct PaymentChange {
    pub id: i64,
    pub created_at: String,
    pub state: Option<String>,
    pub captured: Option<Value>,
    pub refunded: Option<Value>,
}

/// The latest 100 payment changes of an order, newest first.
pub async fn payment_history<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    order: &str,
) -> Result<Vec<PaymentChange>> {
    Ok(sqlx::query_as!(
        PaymentChange,
        r#"SELECT id,to_jsonb(created_at)#>>'{}' AS "created_at!",after_state#>>'{payment,state}' AS state,after_state#>'{payment,capturedMinor}' AS captured,after_state#>'{payment,refundedMinor}' AS refunded FROM entity_history WHERE tenant=$1 AND entity='order' AND entity_id=$2 AND finalized AND (before_state->'payment') IS DISTINCT FROM (after_state->'payment') ORDER BY id DESC LIMIT 100"#,
        tenant,
        order
    )
    .fetch_all(db)
    .await?)
}
