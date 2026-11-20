//! The installed package of a shop, read on the backend the caller already holds.
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, Result, StatusCode, bad, conflict};

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
