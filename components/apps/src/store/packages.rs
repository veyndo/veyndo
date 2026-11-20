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
