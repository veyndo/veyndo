//! Tenant-local dependency and version gates of a signed package, read under the installing transaction.
use crate::domain::distribution::verify;
use crate::domain::model_contract;
use serde_json::Value;
use veyndo_app_model::distribution as shared;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad, conflict};

/// Requires the active dependencies of a signed package in admitted versions and keeps installed dependants satisfied.
pub async fn check(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    publishers: &Value,
) -> Result<()> {
    let Some(d) = &m.distribution else {
        return Ok(());
    };
    for dep in &d.dependencies {
        let row = sqlx::query!(
            "SELECT manifest,version FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
            t,
            dep.app
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(conflict(
            "Install and activate required app dependencies first",
        ))?;
        let prior: Manifest =
            serde_json::from_value(row.manifest).map_err(|_| bad("Invalid dependency package"))?;
        verify(&prior, publishers)?;
        shared::require_version(dep, &prior, &row.version).map_err(model_contract::error)?;
    }
    // Upgrading an existing dependency cannot silently break active installed dependants.
    let installed: Vec<Value> = sqlx::query_scalar!(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id<>$2 AND active",
        t,
        m.id
    )
    .fetch_all(&mut **tx)
    .await?;
    shared::validate_graph(m, &installed).map_err(model_contract::error)
}
