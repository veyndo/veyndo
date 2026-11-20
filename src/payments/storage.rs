//! Transactional provider receipts and order state updates; external responses cannot invent amounts or tenants.
use super::{Attempt, generic_receipts};
use crate::{Result, Value};

#[path = "../../extensions/apps/paypal/src/receipts.rs"]
mod legacy_receipts;

pub(crate) async fn persist(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    op: &str,
    input: &Value,
    v: &Value,
) -> Result<()> {
    if p.provider == "paypal" {
        legacy_receipts::persist(tx, p, op, input, v).await
    } else {
        generic_receipts::persist(tx, p, op, input, v).await
    }
}
pub(crate) async fn update_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    state: &str,
) -> Result<()> {
    veyndo_payments::store::order_ledger::update_order(&super::host::PaymentHost, tx, p, state)
        .await
}
