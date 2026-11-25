//! SQL of the merchant order workflow: its write lock, revision, source app and the check that no active order is stranded.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::Result;

/// Serializes workflow saves of one shop.
pub async fn lock(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,27))", t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Whether the app is installed and active.
pub async fn app_active(tx: &mut Tx<'_>, t: &str, app: &str) -> Result<bool> {
    veyndo_app_platform::store::workspace::app_active(tx, t, app).await
}

/// The stored workflow's revision, locked; `None` while the default workflow applies.
pub async fn revision(tx: &mut Tx<'_>, t: &str) -> Result<Option<i64>> {
    veyndo_orders::store::workflow::revision(tx, t).await
}

/// Whether an active order outside the native lifecycle is in a state the workflow no longer has.
pub async fn strands_legacy_orders(tx: &mut Tx<'_>, t: &str, states: &[String]) -> Result<bool> {
    veyndo_orders::store::workflow::strands_legacy_orders(tx, t, states).await
}

/// Inserts or replaces the order workflow.
pub async fn save(tx: &mut Tx<'_>, t: &str, value: &Value) -> Result<()> {
    veyndo_orders::store::workflow::save(tx, t, value).await
}
