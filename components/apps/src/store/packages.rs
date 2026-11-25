//! The installed package of a shop, read on the backend the caller already holds.
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};

/// Lifecycle transactions use their current backend without recursively taking a pooled connection.
pub async fn package_on<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    t: &str,
    id: &str,
    active: bool,
) -> Result<Manifest> {
    let row = sqlx::query("SELECT manifest,active FROM app_packages WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "App not installed".into()))?;
    if active && !row.get::<bool, _>("active") {
        return Err(conflict("App is inactive"));
    }
    serde_json::from_value(row.get("manifest")).map_err(|_| bad("Invalid installed manifest"))
}
/// Every active package of the shop a request addresses.
pub async fn active_manifests(a: &State, h: &RequestContext) -> Result<Vec<Manifest>> {
    let rows =
        sqlx::query("SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id")
            .bind(tenant(h)?)
            .fetch_all(&a.db)
            .await?;
    rows.into_iter()
        .map(|r| serde_json::from_value(r.get("manifest")).map_err(|_| bad("Invalid app package")))
        .collect()
}
/// One installed package of a shop; an inactive one is refused when the caller needs it active.
pub async fn package(a: &State, t: &str, id: &str, active: bool) -> Result<Manifest> {
    package_on(&a.db, t, id, active).await
}
/// The stored manifests of every active package of a shop, by app ID; callers parse them with their own refusal.
pub async fn active_manifest_values(a: &State, t: &str) -> Result<Vec<serde_json::Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id",
        t
    )
    .fetch_all(&a.db)
    .await?)
}
/// The manifest of an active package, share-locked against concurrent upgrades.
pub async fn active_manifest_for_share(
    conn: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Option<serde_json::Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}
/// The digest of an active package, share-locked against concurrent upgrades.
pub async fn active_digest_for_share(
    conn: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT digest FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}
/// The stored manifest of a package, active or not.
pub async fn manifest_value(
    conn: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Option<serde_json::Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}
/// A stored package with its activity flag.
pub struct StoredPackage {
    pub manifest: serde_json::Value,
    pub active: bool,
}
/// The stored manifest of a package with whether it is active.
pub async fn stored_package(
    conn: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Option<StoredPackage>> {
    Ok(sqlx::query_as!(
        StoredPackage,
        "SELECT manifest,active FROM app_packages WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}
/// How many packages a shop holds besides `id`.
pub async fn count_other(conn: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) AS \"count!\" FROM app_packages WHERE tenant=$1 AND id<>$2",
        t,
        id
    )
    .fetch_one(conn)
    .await?)
}
/// The record count of every model of a package.
pub struct ModelRecords {
    pub entity: String,
    pub records: i64,
}
/// Counts the records of each model of a package, by model name.
pub async fn model_records(
    conn: &mut sqlx::PgConnection,
    t: &str,
    app: &str,
) -> Result<Vec<ModelRecords>> {
    Ok(sqlx::query_as!(
        ModelRecords,
        "SELECT entity,count(*)::bigint AS \"records!\" FROM app_records WHERE tenant=$1 AND app=$2 GROUP BY entity ORDER BY entity",
        t,
        app
    )
    .fetch_all(conn)
    .await?)
}
