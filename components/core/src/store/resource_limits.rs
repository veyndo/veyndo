//! Per-shop resource limits that admission enforces: the stored daily AI limit and its revision.
use crate::{Result, State};
use sqlx::PgConnection;

/// A shop's daily interactive AI limit and its revision.
pub struct Limit {
    pub daily_ai: i64,
    pub revision: i64,
}

/// The stored AI limit of a shop; none while it uses the defaults.
pub async fn limit(a: &State, tenant: &str) -> Result<Option<Limit>> {
    Ok(sqlx::query_as!(
        Limit,
        "SELECT daily_ai,revision FROM tenant_resource_limits WHERE tenant=$1",
        tenant
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The revision of a shop's stored AI limit, on the caller's connection.
pub async fn revision(conn: &mut PgConnection, tenant: &str) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT revision FROM tenant_resource_limits WHERE tenant=$1",
        tenant
    )
    .fetch_optional(conn)
    .await?)
}

/// Inserts the limit at `revision`, or replaces it and advances the stored revision.
pub async fn upsert(
    conn: &mut PgConnection,
    tenant: &str,
    daily_ai: i64,
    revision: i64,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO tenant_resource_limits(tenant,daily_ai,revision) VALUES($1,$2,$3) ON CONFLICT(tenant) DO UPDATE SET daily_ai=excluded.daily_ai,revision=tenant_resource_limits.revision+1",
        tenant,
        daily_ai,
        revision
    )
    .execute(conn)
    .await?;
    Ok(())
}
