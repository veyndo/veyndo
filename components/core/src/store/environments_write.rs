//! Writes of the staging registry (`shop_environments`): registering a stage, replacing its baseline and the preview ownership window.
use crate::Result;
use serde_json::Value;
use sqlx::PgConnection;

/// Registers `tenant` as a stage of the live shop with its name and baseline.
pub async fn register(
    conn: &mut PgConnection,
    tenant: &str,
    live: &str,
    name: &str,
    baseline: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO shop_environments(tenant,live_tenant,name,baseline) VALUES($1,$2,$3,$4)",
        tenant,
        live,
        name,
        baseline
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Replaces the environment's baseline.
pub async fn set_baseline(conn: &mut PgConnection, tenant: &str, baseline: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE shop_environments SET baseline=$1 WHERE tenant=$2",
        baseline,
        tenant
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Marks the environment as a developer preview owned by one actor for one hour.
pub async fn mark_preview(conn: &mut PgConnection, tenant: &str, owner: &str) -> Result<()> {
    sqlx::query!("UPDATE shop_environments SET preview_owner=$1,preview_until=now()+interval '1 hour' WHERE tenant=$2", owner, tenant)
        .execute(conn)
        .await?;
    Ok(())
}

/// Keeps a preview environment for another hour.
pub async fn extend_preview(conn: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE shop_environments SET preview_until=now()+interval '1 hour' WHERE tenant=$1",
        tenant
    )
    .execute(conn)
    .await?;
    Ok(())
}
