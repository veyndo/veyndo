//! The expiry of personal preview environments that app previews keep alive.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Keeps a preview environment for another hour.
pub async fn extend(conn: &mut PgConnection, environment: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE shop_environments SET preview_until=now()+interval '1 hour' WHERE tenant=$1",
        environment
    )
    .execute(conn)
    .await?;
    Ok(())
}
