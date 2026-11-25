//! The shop's active app packages with their manifests, as the automation catalogue lists them through marketing's records port. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// An installed, active app package and its manifest.
pub struct ActivePackage {
    pub id: String,
    pub manifest: Value,
}

/// The shop's active app packages by id.
pub async fn active(a: &State, tenant: &str) -> Result<Vec<ActivePackage>> {
    Ok(sqlx::query_as!(
        ActivePackage,
        "SELECT id,manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}
