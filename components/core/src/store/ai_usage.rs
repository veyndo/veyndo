//! The daily AI attempt counter of a shop; a stage counts against its live shop.
use crate::Result;

/// Counts one AI attempt for today when the shop is below `default_maximum` (or its own limit); `None` when the quota is reached.
pub async fn reserve(
    connection: &mut sqlx::PgConnection,
    tenant: &str,
    default_maximum: i64,
) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!("WITH owner AS (SELECT coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) AS tenant), scope AS (SELECT tenant,coalesce((SELECT daily_ai FROM tenant_resource_limits l WHERE l.tenant=owner.tenant),$2) AS maximum FROM owner) INSERT INTO tenant_ai_usage(tenant,day,attempts) SELECT tenant,(now() AT TIME ZONE 'UTC')::date,1 FROM scope WHERE maximum>0 ON CONFLICT(tenant,day) DO UPDATE SET attempts=tenant_ai_usage.attempts+1 WHERE tenant_ai_usage.attempts < (SELECT maximum FROM scope) RETURNING attempts", tenant, default_maximum)
        .fetch_optional(connection)
        .await?)
}
