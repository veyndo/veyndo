//! Transaction-local settings of app writes: advisory locks, the tenant row scope, review timeouts and deferred constraints.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Serialises writers of one key for the rest of the transaction; `seed` keeps unrelated lock families apart.
pub async fn advisory(conn: &mut PgConnection, key: &str, seed: i64) -> Result<()> {
    sqlx::query!(
        "SELECT 1 AS \"locked!\" FROM pg_advisory_xact_lock(hashtextextended($1,$2))",
        key,
        seed
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Scopes row security to one shop for the rest of the transaction.
pub async fn tenant_scope(conn: &mut PgConnection, tenant: &str) -> Result<()> {
    sqlx::query!("SELECT set_config('rac.tenant',$1,true)", tenant)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// Bounds lock waits and statements of a dry-run review transaction.
pub async fn review_timeouts(conn: &mut PgConnection) -> Result<()> {
    sqlx::query!(
        "SELECT set_config('lock_timeout','2s',true) AS lock_timeout,set_config('statement_timeout','10s',true) AS statement_timeout"
    )
    .fetch_one(conn)
    .await?;
    Ok(())
}

/// Checks deferred relational constraints now, so a rolled-back dry run still sees them.
pub async fn constraints_immediate(conn: &mut PgConnection) -> Result<()> {
    sqlx::query!("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(conn)
        .await?;
    Ok(())
}
