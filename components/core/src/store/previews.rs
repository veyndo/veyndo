//! Reads of the staging registry (`shop_environments`) for environments apps preview and install in: ownership, personal previews and private targets.
use crate::Result;
use sqlx::PgConnection;

/// Whether `environment` belongs to the live shop `tenant`.
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
pub async fn preview_of(a: &crate::State, live: &str, owner: &str) -> Result<Option<String>> {
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

/// Share-locks a shared (not personal) environment of the live shop; `false` when it is none.
pub async fn lock_shared(conn: &mut PgConnection, target: &str, live: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT tenant FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 AND preview_owner IS NULL FOR SHARE",
        target,
        live
    )
    .fetch_optional(conn)
    .await?
    .is_some())
}

/// Whether the shop is a personal preview environment.
pub async fn is_personal(conn: &mut PgConnection, t: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NOT NULL) AS "private!""#,
        t
    )
    .fetch_one(conn)
    .await?)
}
