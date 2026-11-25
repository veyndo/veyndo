//! The shop environments apps preview in: ownership by a live shop and the personal preview of one merchant.
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// Whether `environment` belongs to the live shop `tenant`; read on the caller's scoped connection.
pub async fn owned(
    conn: &mut PgConnection,
    tenant: &str,
    environment: &str,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM shop_environments WHERE live_tenant=$1 AND tenant=$2) AS "owned!""#,
        tenant,
        environment
    )
    .fetch_one(conn)
    .await
}

/// The personal preview environment of a merchant in a live shop, if one exists.
pub async fn preview_of(a: &State, live: &str, owner: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT tenant FROM shop_environments WHERE live_tenant=$1 AND preview_owner=$2",
        live,
        owner
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Locks the personal preview environment; fails when it is no longer this merchant's preview.
pub async fn lock_preview(
    conn: &mut PgConnection,
    environment: &str,
    live: &str,
    owner: &str,
) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT tenant FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 AND preview_owner=$3 FOR UPDATE",
        environment,
        live,
        owner
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Keeps a preview environment for another hour.
pub async fn extend_preview(conn: &mut PgConnection, environment: &str) -> Result<()> {
    veyndo_core::store::environments_write::extend_preview(conn, environment).await
}
