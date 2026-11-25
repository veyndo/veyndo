//! Integration-key admission of agents: whether a key may still act for the shop's live shop while agents are not stopped.
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Whether an integration key may still act for an agent: unexpired, on the shop's live shop, agents not stopped.
pub async fn key_permitted(e: impl PgExecutor<'_>, key: &str, tenant: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM integration_keys i JOIN tenants t ON t.id=i.tenant WHERE i.id=$1 AND i.tenant=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$2),$2) AND i.expires_at>now() AND NOT t.agents_stopped) AS "permitted!""#,
        key, tenant).fetch_one(e).await?)
}
