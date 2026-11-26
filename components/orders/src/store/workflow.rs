//! The stored order workflow: its locked revision, the stranded-order check and saves at the next revision.
use serde_json::Value;
use veyndo_core::Result;

/// The stored workflow's revision, locked; `None` while the default workflow applies.
pub async fn revision(c: &mut sqlx::PgConnection, t: &str) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM order_state_machines WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_optional(&mut *c)
    .await?)
}

/// Whether an active order outside the native lifecycle is in a state the workflow no longer has.
pub async fn strands_legacy_orders(
    c: &mut sqlx::PgConnection,
    t: &str,
    states: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM orders WHERE tenant=$1 AND NOT (data ? 'nativeLifecycle') AND NOT (data->>'state'=ANY($2)) AND data->>'state' NOT IN ('expired','payment_review')) AS "exists!""#, t, states).fetch_one(&mut *c).await?)
}

/// Inserts or replaces the order workflow.
pub async fn save(c: &mut sqlx::PgConnection, t: &str, value: &Value) -> Result<()> {
    sqlx::query!("INSERT INTO order_state_machines(tenant,data) VALUES($1,$2) ON CONFLICT(tenant) DO UPDATE SET data=EXCLUDED.data,revision=order_state_machines.revision+1", t, value).execute(&mut *c).await?;
    Ok(())
}
