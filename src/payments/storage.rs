//! Transactional provider receipts and order state updates; external responses cannot invent amounts or tenants.
use super::{Attempt, generic_receipts};
use serde_json::Value;
use veyndo_core::Result;

pub(crate) async fn persist(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    op: &str,
    input: &Value,
    v: &Value,
) -> Result<()> {
    if p.provider == "paypal" {
        veyndo_paypal::receipts::persist(&super::host::PaymentHost, tx, p, op, input, v).await
    } else {
        generic_receipts::persist(tx, p, op, input, v).await
    }
}
