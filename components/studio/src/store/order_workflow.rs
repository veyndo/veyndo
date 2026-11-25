//! SQL of the merchant order workflow: its write lock, revision, source app and the check that no active order is stranded.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::Result;

/// Serializes workflow saves of one shop.
pub async fn lock(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,27))")
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Whether the app is installed and active.
pub async fn app_active(tx: &mut Tx<'_>, t: &str, app: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_packages WHERE tenant=$1 AND id=$2 AND active)",
    )
    .bind(t)
    .bind(app)
    .fetch_one(&mut **tx)
    .await?)
}

/// The stored workflow's revision, locked; `None` while the default workflow applies.
pub async fn revision(tx: &mut Tx<'_>, t: &str) -> Result<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT revision FROM order_state_machines WHERE tenant=$1 FOR UPDATE")
            .bind(t)
            .fetch_optional(&mut **tx)
            .await?,
    )
}

/// Whether an active order outside the native lifecycle is in a state the workflow no longer has.
pub async fn strands_legacy_orders(tx: &mut Tx<'_>, t: &str, states: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND NOT (data ? 'nativeLifecycle') AND NOT (data->>'state'=ANY($2)) AND data->>'state' NOT IN ('expired','payment_review'))").bind(t).bind(states).fetch_one(&mut **tx).await?)
}

/// Inserts or replaces the order workflow.
pub async fn save(tx: &mut Tx<'_>, t: &str, value: &Value) -> Result<()> {
    sqlx::query("INSERT INTO order_state_machines(tenant,data) VALUES($1,$2) ON CONFLICT(tenant) DO UPDATE SET data=EXCLUDED.data,revision=order_state_machines.revision+1").bind(t).bind(value).execute(&mut **tx).await?;
    Ok(())
}
