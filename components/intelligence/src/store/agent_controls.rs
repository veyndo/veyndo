//! The live shop's agent stop switch and integration-key admission in storage. Columns checked at compile time.
use sqlx::{PgConnection, PgExecutor};
use veyndo_core::Result;
pub use veyndo_core::store::agent_switch::AgentSwitch;

/// Whether an integration key may still act for an agent: unexpired, on the shop's live shop, agents not stopped.
pub async fn key_permitted(e: impl PgExecutor<'_>, key: &str, tenant: &str) -> Result<bool> {
    veyndo_identity::store::agent_keys::key_permitted(e, key, tenant).await
}

/// The live shop of a shop or sandbox.
pub async fn live_shop(e: impl PgExecutor<'_>, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) AS "live!""#,
        tenant
    )
    .fetch_one(e)
    .await?)
}

/// The live shop's stop switch, locked against concurrent changes.
pub async fn switch_locked(e: impl PgExecutor<'_>, live: &str) -> Result<AgentSwitch> {
    veyndo_core::store::agent_switch::switch_locked(e, live).await
}

/// Sets the stop switch and advances its revision.
pub async fn set_stopped(e: impl PgExecutor<'_>, live: &str, stopped: bool) -> Result<()> {
    veyndo_core::store::agent_switch::set_stopped(e, live, stopped).await
}

/// Cancels every active run of the live shop and its sandboxes and releases their conversation turns; returns the cancelled count.
pub async fn cancel_all(tx: &mut PgConnection, live: &str) -> Result<u64> {
    let count=sqlx::query!("UPDATE tasks SET run_state='cancelled',run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE (tenant=$1 OR tenant IN(SELECT tenant FROM shop_environments WHERE live_tenant=$1)) AND run_state IN('queued','running','uncertain')",
        live).execute(&mut *tx).await?.rows_affected();
    sqlx::query!("UPDATE conversations c SET turn_owner=NULL,turn_until=NULL WHERE (tenant=$1 OR tenant IN(SELECT tenant FROM shop_environments WHERE live_tenant=$1)) AND EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=c.tenant AND k.id=c.turn_owner AND k.run_state='cancelled')",
        live).execute(&mut *tx).await?;
    Ok(count)
}
