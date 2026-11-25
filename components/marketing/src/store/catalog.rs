//! What the automation catalogue lists from the shop: active app manifests and active rules. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// An installed, active app package and its manifest.
pub struct ActiveApp {
    pub id: String,
    pub manifest: Value,
}

/// The shop's active app packages by id.
pub async fn active_apps(a: &State, tenant: &str) -> Result<Vec<ActiveApp>> {
    Ok(sqlx::query_as!(
        ActiveApp,
        "SELECT id,manifest FROM app_packages WHERE tenant=$1 AND active ORDER BY id",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// An active rule a condition may reference.
pub struct ActiveRule {
    pub id: String,
    pub name: Value,
    pub revision: i64,
}

/// The first hundred active rules by id.
pub async fn active_rules(a: &State, tenant: &str) -> Result<Vec<ActiveRule>> {
    Ok(sqlx::query_as!(
        ActiveRule,
        "SELECT id,name,revision FROM commerce_rules WHERE tenant=$1 AND active ORDER BY id LIMIT 100",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}
