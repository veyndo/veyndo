//! SQL of staging snapshots and product publication: locked reads of every publishable unit and the native product content writes.
use super::staging_documents::Tx;
use serde_json::Value;
pub use veyndo_app_platform::store::staging::Package;
pub use veyndo_catalog::store::staging::NewProduct;
use veyndo_core::Result;

/// Locks the settings row first, in the same order as native organization writes.
pub async fn lock_settings(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    veyndo_shop::store::staging::lock_settings(tx, t).await
}

/// Every product as a document without tenant, stock and revision clocks, locked.
pub async fn products(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Value>> {
    veyndo_catalog::store::staging::products(tx, t).await
}

/// One product as a document without tenant, stock and revision clocks, locked.
pub async fn product(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Value> {
    veyndo_catalog::store::staging::product(tx, t, id).await
}

/// The settings document, locked.
pub async fn settings(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    veyndo_shop::store::staging::locked_settings(tx, t).await
}

/// The storefront experience document, locked.
pub async fn experience(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    veyndo_intelligence::store::staging::experience(tx, t).await
}

/// Every installed app package, locked.
pub async fn packages(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Package>> {
    veyndo_app_platform::store::staging::packages(tx, t).await
}

/// The release units of one kind (`rule`, `promotion`, `flow`, `channel`) by ID, locked.
pub async fn units(tx: &mut Tx<'_>, t: &str, kind: &str) -> Result<Vec<(String, Value)>> {
    veyndo_marketing::store::staging::units(tx, t, kind).await
}

/// The order workflow document, locked.
pub async fn order_workflow(tx: &mut Tx<'_>, t: &str) -> Result<Option<Value>> {
    veyndo_orders::store::staging::order_workflow(tx, t).await
}

/// A product's translations as `(language, name, description)`, locked.
pub async fn translations(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    veyndo_catalog::store::staging::translations(tx, t, id).await
}

/// A product's category IDs.
pub async fn category_ids(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Vec<String>> {
    veyndo_catalog::store::staging::category_ids(tx, t, id).await
}

/// A product's channel visibility as `(channel, visible)`.
pub async fn channel_visibility(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Vec<(String, bool)>> {
    veyndo_catalog::store::staging::channel_visibility(tx, t, id).await
}

/// Inserts a product with zero inventory unless it already exists.
pub async fn insert_product(tx: &mut Tx<'_>, t: &str, id: &str, p: &NewProduct<'_>) -> Result<()> {
    veyndo_catalog::store::staging::insert_product(tx, t, id, p).await
}

/// Overwrites a product's content columns from a document through an explicit allowlist; returns the rows changed.
pub async fn update_product_content(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    value: &Value,
) -> Result<u64> {
    veyndo_catalog::store::staging::update_product_content(tx, t, id, value).await
}

/// Removes a product's channel visibility rows.
pub async fn clear_channel_visibility(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    veyndo_catalog::store::staging::clear_channel_visibility(tx, t, id).await
}

/// Adds one channel visibility row of a product.
pub async fn insert_channel_visibility(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    channel: Option<&str>,
    visible: Option<bool>,
) -> Result<()> {
    veyndo_catalog::store::staging::insert_channel_visibility(tx, t, id, channel, visible).await
}

/// Inserts or replaces one product translation.
pub async fn upsert_translation(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    language: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    veyndo_catalog::store::staging::upsert_translation(tx, t, id, language, name, description).await
}

/// Whether a product exists in the shop.
pub async fn product_exists(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<bool> {
    veyndo_catalog::store::staging::product_exists(tx, t, id).await
}
