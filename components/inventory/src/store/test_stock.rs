//! Stock writes for empty purchasable products, in bounded batches.
use sqlx::PgConnection;
use veyndo_catalog::store::products::ProductRow;
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
    Ok(sqlx::query_as!(ProductRow, r#"UPDATE products p SET stock=$2,revision=revision+1 WHERE tenant=$1 AND id IN (SELECT id FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id) ORDER BY e.id LIMIT $3 FOR UPDATE) RETURNING id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number"#, tenant, quantity, batch)
        .fetch_all(tx)
        .await?
        .into_iter()
        .map(|row| Filled {
            revision: row.revision,
            product: row.into(),
        })
        .collect())
}

/// Active, empty products without variants that are still left.
pub async fn remaining(tx: &mut PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id)"#, tenant)
        .fetch_one(tx)
        .await?)
}
