//! Provider-neutral receipt admission and atomic allocation; verified evidence owns ledger transitions.
use super::Attempt;
use serde_json::Value;
use veyndo_core::Result;

pub(crate) async fn persist(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    op: &str,
    input: &Value,
    v: &Value,
) -> Result<()> {
    veyndo_payments::service::receipts::persist(&super::host::PaymentHost, tx, p, op, input, v)
        .await
}
