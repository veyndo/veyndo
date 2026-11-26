//! SQL of reviewed migration imports: idempotent replays of already imported records, the native state inspections compare against and historical order inserts.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::{Result, State};
pub use veyndo_customers::store::imports::Customer;
pub use veyndo_orders::store::imports::HistoricalOrder;

/// A category's data document.
pub async fn category_data(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::category_data(a, t, id).await
}

/// A category as `{parentId,data}`.
pub async fn category(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::category(a, t, id).await
}

/// A promotion's data document.
pub async fn promotion(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_marketing::store::imports::promotion(a, t, id).await
}

/// A product's `extra` document.
pub async fn product_extra(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::product_extra(a, t, id).await
}

/// A product's media list.
pub async fn product_media(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::product_media(a, t, id).await
}

/// An asset's metadata with the digest and length of its content.
pub async fn asset(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::asset(a, t, id).await
}

/// A customer's imported fields.
pub async fn customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    veyndo_customers::store::imports::customer(a, t, id).await
}

/// An order's data with its UTC creation time as `sourceCreatedAt`.
pub async fn order(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_orders::store::imports::order(a, t, id).await
}

/// A review in its import shape.
pub async fn review(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_catalog::store::imports::review(a, t, id).await
}

/// Serializes imports of one historical source identity (`<tenant>:<key>`).
pub async fn lock_historical(tx: &mut Tx<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,28))", key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The fingerprint of an order already imported under the idempotency key.
pub async fn imported_fingerprint(tx: &mut Tx<'_>, t: &str, key: &str) -> Result<Option<String>> {
    veyndo_orders::store::imports::imported_fingerprint(tx, t, key).await
}

/// Share-locks a sales channel; `false` when it does not exist.
pub async fn share_channel(tx: &mut Tx<'_>, t: &str, channel: &str) -> Result<bool> {
    veyndo_marketing::store::imports::share_channel(tx, t, channel).await
}

/// Inserts a historical order, its completed cart and its import activity.
pub async fn insert_historical(tx: &mut Tx<'_>, t: &str, o: HistoricalOrder<'_>) -> Result<()> {
    veyndo_orders::store::imports::insert_historical(tx, t, o).await
}
