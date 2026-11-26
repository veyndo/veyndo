//! SQL of the studio's product reads and writes: shared product rows for proposals and the editor, reviewed price/stock changes and existence checks.
use super::staging_documents::Tx;
use crate::domain::proposal::Change;
use serde_json::Value;
use veyndo_catalog::store::workspace as catalog;
pub use veyndo_catalog::store::workspace::EditorProduct;
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// Up to 100 of the requested products.
pub async fn requested(a: &State, t: &str, ids: &[String]) -> Result<Vec<Product>> {
    catalog::requested(a, t, ids).await
}

/// The given products, locked in ID order.
pub async fn lock(tx: &mut Tx<'_>, t: &str, ids: Vec<String>) -> Result<Vec<Product>> {
    catalog::lock(tx, t, &ids).await
}

/// One product inside a transaction.
pub async fn get(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Product> {
    catalog::get(tx, t, id).await
}

/// The product columns the editor shows.
pub async fn editor_row(a: &State, t: &str, id: &str) -> Result<Option<EditorProduct>> {
    catalog::editor_product(a, t, id).await
}

/// A product's translation in each given locale as `(locale, name, description)`; missing ones are `None`.
pub async fn translations(
    a: &State,
    t: &str,
    id: &str,
    locales: &[String],
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    catalog::translations(a, t, id, locales).await
}

/// Applies a reviewed price/stock change at its expected revision; returns the rows changed.
pub async fn apply_change(tx: &mut Tx<'_>, t: &str, c: &Change) -> Result<u64> {
    catalog::apply_change(
        tx,
        t,
        (&c.product_id, c.expected_revision),
        c.price,
        c.stock,
    )
    .await
}

/// Whether a product exists.
pub async fn exists(a: &State, t: &str, id: &str) -> Result<bool> {
    catalog::exists(a, t, id).await
}

/// Whether a category exists.
pub async fn category_exists(a: &State, t: &str, id: &str) -> Result<bool> {
    catalog::category_exists(a, t, id).await
}

/// Replaces the storefront experience at its expected revision; returns the rows changed.
pub async fn replace_experience(
    tx: &mut Tx<'_>,
    t: &str,
    data: Value,
    expected: Option<i64>,
) -> Result<u64> {
    veyndo_intelligence::store::workspace::replace_experience(tx, t, data, expected).await
}

/// The settings document and its revision, locked for update.
pub async fn lock_settings(tx: &mut Tx<'_>, t: &str) -> Result<(Value, i64)> {
    veyndo_shop::store::workspace::lock_settings(tx, t).await
}

/// The settings document and its revision, share-locked.
pub async fn share_settings(tx: &mut Tx<'_>, t: &str) -> Result<(Value, i64)> {
    veyndo_shop::store::workspace::share_settings(tx, t).await
}
