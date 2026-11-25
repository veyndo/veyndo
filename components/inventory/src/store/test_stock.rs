//! Stock writes for empty purchasable products, in bounded batches.
use sqlx::PgConnection;
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;

/// A product whose stock was just set, with its new revision.
pub struct Filled {
    pub product: Product,
    pub revision: i64,
}

/// Gives up to `batch` active, empty products without variants the quantity, in ID order.
pub async fn fill(
    tx: &mut PgConnection,
    tenant: &str,
    quantity: i32,
    batch: i64,
) -> Result<Vec<Filled>> {
    Ok(
        veyndo_catalog::store::stock::fill_empty(tx, tenant, quantity, batch)
            .await?
            .into_iter()
            .map(|row| Filled {
                revision: row.revision,
                product: row.into(),
            })
            .collect(),
    )
}

/// Active, empty products without variants that are still left.
pub async fn remaining(tx: &mut PgConnection, tenant: &str) -> Result<i64> {
    veyndo_catalog::store::stock::empty_count(tx, tenant).await
}
