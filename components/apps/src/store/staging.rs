//! App packages and extensions of a sandbox shop: copying, locked snapshot reads and release activation, inside the caller's transaction.
use serde_json::Value;
use veyndo_core::Result;

/// One installed app package.
pub struct Package {
    pub id: String,
    pub manifest: Value,
    pub active: bool,
}

/// Manifests of the live shop's active apps, by ID.
pub async fn active_manifests(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id",
        t
    )
    .fetch_all(&mut *c)
    .await?)
}

/// Every installed app package, locked.
pub async fn packages(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<Package>> {
    Ok(sqlx::query_as!(
        Package,
        "SELECT id,manifest,active FROM app_packages WHERE tenant=$1 ORDER BY id FOR UPDATE",
        t
    )
    .fetch_all(&mut *c)
    .await?)
}

/// Sets whether an installed app package is active.
pub async fn set_app_active(
    c: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
    active: Option<bool>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE app_packages SET active=$1 WHERE tenant=$2 AND id=$3",
        active,
        t,
        app
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// The manifest of an installed app package.
pub async fn manifest(c: &mut sqlx::PgConnection, t: &str, app: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2",
        t,
        app
    )
    .fetch_optional(&mut *c)
    .await?)
}

/// Installs the sandbox's company-limit extension.
pub async fn insert_extension(
    c: &mut sqlx::PgConnection,
    id: &str,
    wat: &str,
    digest: &str,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO extensions(tenant,wat,digest) VALUES($1,$2,$3)",
        id,
        wat,
        digest
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}
