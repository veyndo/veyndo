//! The live shop's agent stop switch on the shop row: the locked read and the revisioned update.
use crate::Result;
use sqlx::PgExecutor;

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
