//! The live shop behind a shop or one of its staging/preview environments: its ID and agent stop switch.
use crate::Result;
use sqlx::{Executor, PgConnection, Postgres};

/// The live shop of a tenant or its environment, with its agent stop switch.
pub struct LiveShop {
    pub id: String,
    pub agents_stopped: bool,
}

/// The ID of the live shop behind a shop or environment; a live shop is its own.
pub async fn resolve<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<String> {
    Ok(sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) AS "id!""#,
        tenant
    )
    .fetch_one(db)
    .await?)
}

/// Locks the live shop row against concurrent stop changes and returns it.
pub async fn lock(conn: &mut PgConnection, tenant: &str) -> Result<LiveShop> {
    Ok(sqlx::query_as!(
        LiveShop,
        "SELECT id,agents_stopped FROM tenants WHERE id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) FOR NO KEY UPDATE",
        tenant
    )
    .fetch_one(conn)
    .await?)
}
