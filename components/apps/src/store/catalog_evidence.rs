//! Product-only app callback projects current catalog text and enabled translations in one snapshot.
use serde_json::{Value, json};
use veyndo_core::{Result, State};
use veyndo_shop::service::read_cache::config;

/// The current catalogue text of a product with its enabled translations, in one snapshot.
pub async fn catalog_evidence(a: &State, tenant: &str, id: &str) -> Result<Value> {
    let (settings, _) = config(a, tenant).await?;
    let product = veyndo_catalog::store::app_references::evidence(
        &a.db,
        tenant,
        id,
        &settings.main_locale,
        &settings.locales,
    )
    .await?;
    Ok(product.unwrap_or_else(|| json!({"id":id,"active":false})))
}
