//! Which categories a product is assigned to.
use sqlx::PgConnection;
use veyndo_core::{Result, State};

pub async fn exists(tx: &mut PgConnection, tenant: &str, category: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2)")
            .bind(tenant)
            .bind(category)
            .fetch_one(tx)
            .await?,
    )
}

/// Replaces the assignments of one product.
pub async fn replace(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    categories: &[String],
) -> Result<()> {
    sqlx::query("DELETE FROM product_categories WHERE tenant=$1 AND product_id=$2")
        .bind(tenant)
        .bind(product)
        .execute(&mut *tx)
        .await?;
    for category in categories {
        sqlx::query("INSERT INTO product_categories VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(tenant)
            .bind(product)
            .bind(category)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

pub async fn of_product(a: &State, tenant: &str, product: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT category_id FROM product_categories WHERE tenant=$1 AND product_id=$2 ORDER BY category_id")
        .bind(tenant)
        .bind(product)
        .fetch_all(&a.db)
        .await?)
}
