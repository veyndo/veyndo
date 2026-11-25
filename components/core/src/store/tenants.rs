//! Reads and locks of the shop registry every component shares: existence, name, lifecycle status, the lifecycle lock and the agent stop switch of the live shop.
use crate::Result;
use sqlx::{Executor, Postgres};

/// Locks the shop row; every membership, key and lifecycle change of the shop serializes on it. A missing shop is an error.
pub async fn lock<'e>(db: impl Executor<'e, Database = Postgres>, tenant: &str) -> Result<()> {
    sqlx::query!("SELECT id FROM tenants WHERE id=$1 FOR UPDATE", tenant)
        .fetch_one(db)
        .await?;
    Ok(())
}

/// Locks the shop row when the shop exists.
pub async fn lock_if_exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT id FROM tenants WHERE id=$1 FOR UPDATE", tenant)
            .fetch_optional(db)
            .await?,
    )
}

/// Whether a shop with this ID exists.
pub async fn exists<'e>(db: impl Executor<'e, Database = Postgres>, tenant: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM tenants WHERE id=$1) AS "exists!""#,
        tenant
    )
    .fetch_one(db)
    .await?)
}

/// The display name of a shop.
pub async fn name<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT name FROM tenants WHERE id=$1", tenant)
            .fetch_optional(db)
            .await?,
    )
}

/// The lifecycle status of a shop (`active`, `paused`, `archived`).
pub async fn status<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar!("SELECT status FROM tenants WHERE id=$1", tenant)
            .fetch_optional(db)
            .await?,
    )
}

/// Whether an administrator stopped the agents of the live shop behind this shop or sandbox; holds the switch until the transaction ends.
pub async fn agents_stopped_locked<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!("SELECT agents_stopped FROM tenants WHERE id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1) FOR NO KEY UPDATE", tenant).fetch_one(db).await?)
}

/// Whether an administrator stopped the agents of the live shop behind this shop or sandbox.
pub async fn agents_stopped<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!("SELECT agents_stopped FROM tenants WHERE id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1)", tenant).fetch_one(db).await?)
}

/// Every shop's ID and name, by name (the instance administrator's workspace list).
pub async fn all<'e>(db: impl Executor<'e, Database = Postgres>) -> Result<Vec<(String, String)>> {
    Ok(sqlx::query!("SELECT id,name FROM tenants ORDER BY name")
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|row| (row.id, row.name))
        .collect())
}
