//! The shop twin's replay transaction: short bounds against live checkout locks and the recorded demand it projects from.
use super::staging_documents::Tx;
use veyndo_core::Result;

/// Short lock and statement timeouts for the always rolled-back replay transaction.
pub async fn bound(tx: &mut Tx<'_>) -> Result<()> {
    sqlx::query!(
        "SELECT set_config('lock_timeout','250ms',true) AS lock_timeout,set_config('statement_timeout','5s',true) AS statement_timeout"
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(())
}

/// Recorded units per product over the trailing window, without customer data.
pub async fn demand(
    tx: &mut Tx<'_>,
    t: &str,
    ids: &[String],
    days: i32,
) -> Result<Vec<veyndo_orders::store::product_demand::Demand>> {
    veyndo_orders::store::product_demand::trailing(tx, t, ids, days).await
}
