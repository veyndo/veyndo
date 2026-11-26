//! The buyer a checkout grant stands for: an active customer of a live, active shop with a current session. Checked at compile time.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// The customer of a current session (by token digest) of an active live shop.
pub async fn live_buyer<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    digest: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT c.id FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email JOIN tenants t ON t.id=s.tenant WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active AND t.status='active' AND NOT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=s.tenant)",
        tenant,
        digest
    )
    .fetch_optional(db)
    .await?)
}
