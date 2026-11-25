//! The live shop's agent stop switch and integration-key admission in storage. Columns checked at compile time.
use sqlx::{PgConnection, PgExecutor};
use veyndo_core::Result;

/// Whether an integration key may still act for an agent: unexpired, on the shop's live shop, agents not stopped.
pub async fn key_permitted(e: impl PgExecutor<'_>, key: &str, tenant: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM integration_keys i JOIN tenants t ON t.id=i.tenant WHERE i.id=$1 AND i.tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$2),$2) AND i.expires_at>now() AND NOT t.agents_stopped) AS "permitted!""#,
        key, tenant).fetch_one(e).await?)
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

/// The stop switch of a live shop.
pub struct AgentSwitch {
    pub agents_stopped: bool,
    pub agents_revision: i64,
}

/// The live shop's stop switch, locked against concurrent changes.
pub async fn switch_locked(e: impl PgExecutor<'_>, live: &str) -> Result<AgentSwitch> {
    Ok(sqlx::query_as!(
        AgentSwitch,
        "SELECT agents_stopped,agents_revision FROM tenants WHERE id=$1 FOR NO KEY UPDATE",
        live
    )
    .fetch_one(e)
    .await?)
}

/// Sets the stop switch and advances its revision.
pub async fn set_stopped(e: impl PgExecutor<'_>, live: &str, stopped: bool) -> Result<()> {
    sqlx::query!(
        "UPDATE tenants SET agents_stopped=$2,agents_revision=agents_revision+1 WHERE id=$1",
        live,
        stopped
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Cancels every active run of the live shop and its sandboxes and releases their conversation turns; returns the cancelled count.
pub async fn cancel_all(tx: &mut PgConnection, live: &str) -> Result<u64> {
    let count=sqlx::query!("UPDATE tasks SET run_state='cancelled',run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE (tenant=$1 OR tenant IN(SELECT tenant FROM shop_environments WHERE live_tenant=$1)) AND run_state IN('queued','running','uncertain')",
        live).execute(&mut *tx).await?.rows_affected();
    sqlx::query!("UPDATE conversations c SET turn_owner=NULL,turn_until=NULL WHERE (tenant=$1 OR tenant IN(SELECT tenant FROM shop_environments WHERE live_tenant=$1)) AND EXISTS(SELECT 1 FROM tasks k WHERE k.tenant=c.tenant AND k.id=c.turn_owner AND k.run_state='cancelled')",
        live).execute(&mut *tx).await?;
    Ok(count)
}
