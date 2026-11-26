//! Stock quantity writes on product rows for the inventory component: reservations, releases and test stock.
use crate::store::products::ProductRow;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Takes the quantities off the products; the caller already holds their locks in ID order.
pub async fn take(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[&str],
    quantities: &[i32],
) -> Result<()> {
    sqlx::query!(r#"UPDATE products p SET stock=p.stock-i.quantity,revision=p.revision+1 FROM unnest($2::text[],$3::int[]) AS i(id,quantity) WHERE p.tenant=$1 AND p.id=i.id"#, tenant, ids as &[&str], quantities)
        .execute(tx)
        .await?;
    Ok(())
}

/// Locks the products in ID order and puts the quantities back.
pub async fn put_back(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[String],
    quantities: &[i32],
) -> Result<()> {
    sqlx::query!(r#"WITH released AS (SELECT * FROM unnest($2::text[],$3::int[]) AS r(product_id,quantity)), locked AS MATERIALIZED (SELECT p.id FROM products p JOIN released r ON r.product_id=p.id WHERE p.tenant=$1 ORDER BY p.id FOR UPDATE OF p) UPDATE products p SET stock=p.stock+r.quantity,revision=p.revision+1 FROM released r JOIN locked l ON l.id=r.product_id WHERE p.tenant=$1 AND p.id=r.product_id"#, tenant, ids, quantities)
        .execute(tx)
        .await?;
    Ok(())
}

/// Gives up to `batch` active, empty products without variants the quantity, in ID order.
pub async fn fill_empty(
    tx: &mut PgConnection,
    tenant: &str,
    quantity: i32,
    batch: i64,
) -> Result<Vec<ProductRow>> {
    Ok(sqlx::query_as!(ProductRow, r#"UPDATE products p SET stock=$2,revision=revision+1 WHERE tenant=$1 AND id IN (SELECT id FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id) ORDER BY e.id LIMIT $3 FOR UPDATE) RETURNING id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number"#, tenant, quantity, batch)
        .fetch_all(tx)
        .await?)
}

/// Active, empty products without variants that are still left.
pub async fn empty_count(tx: &mut PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM products e WHERE e.tenant=$1 AND e.active AND e.stock<=0 AND NOT EXISTS(SELECT 1 FROM products c WHERE c.tenant=e.tenant AND c.parent_id=e.id)"#, tenant)
        .fetch_one(tx)
        .await?)
}
