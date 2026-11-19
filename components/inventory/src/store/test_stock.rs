//! Stock writes for empty purchasable products, in bounded batches.
use sqlx::{PgConnection, Row};
use veyndo_catalog::store::products::product;
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
    Ok(sqlx::query("UPDATE products p SET stock=$2,revision=revision+1 WHERE tenant=$1 AND id IN (SELECT id FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id) ORDER BY e.id LIMIT $3 FOR UPDATE) RETURNING *")
        .bind(tenant)
        .bind(quantity)
        .bind(batch)
        .fetch_all(tx)
        .await?
        .iter()
        .map(|row| Filled {
            product: product(row),
            revision: row.get("revision"),
        })
        .collect())
}

/// Active, empty products without variants that are still left.
pub async fn remaining(tx: &mut PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id)")
        .bind(tenant)
        .fetch_one(tx)
        .await?)
}
