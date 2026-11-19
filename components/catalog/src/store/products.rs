//! Product rows as the shared product contract, and the bounded locked reads and writes price jobs use.
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// Hydrates the product contract from a complete `products` row.
pub fn product(r: &PgRow) -> Product {
    Product {
        extra: r.get("extra"),
        parent_id: r.get("parent_id"),
        options: r.get("options"),
        media: r.get("media"),
        properties: r.get("properties"),
        delivery_days: r.get("delivery_days"),
        id: r.get("id"),
        product_number: r
            .get::<Option<String>, _>("product_number")
            .unwrap_or_else(|| r.get("id")),
        name: r.get("name"),
        category: r.get("category"),
        description: r.get("description"),
        price: r.get("price"),
        tax_rate: r.get("tax_rate"),
        stock: r.get("stock"),
        revision: r.get("revision"),
        list_price: r.get("list_price"),
        regulation_price: r.get("regulation_price"),
        advanced_prices: serde_json::from_value(r.get::<Value, _>("advanced_prices"))
            .unwrap_or_default(),
        min_purchase: r.get::<i32, _>("min_purchase") as u32,
        purchase_steps: r.get::<i32, _>("purchase_steps") as u32,
        max_purchase: r.get::<Option<i32>, _>("max_purchase").map(|v| v as u32),
        reference_price: r
            .get::<Option<Value>, _>("reference_price")
            .and_then(|v| serde_json::from_value(v).ok()),
    }
}

/// The active main products of the template shop a new shop is seeded from, at most 101.
pub async fn prototype(a: &State, tenant: &str) -> Result<Vec<Product>> {
    Ok(sqlx::query(
        "SELECT * FROM products WHERE tenant=$1 AND parent_id IS NULL AND active ORDER BY id LIMIT 101",
    )
    .bind(tenant)
    .fetch_all(&a.db)
    .await?
    .iter()
    .map(product)
    .collect())
}

/// Up to 100 products after `cursor` up to `ceiling` that existed at `snapshot`, in ID order and locked.
pub async fn lock_range(
    tx: &mut PgConnection,
    tenant: &str,
    cursor: &str,
    ceiling: &str,
    snapshot: &str,
) -> Result<Vec<Product>> {
    Ok(sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id>$2 AND id<=$3 AND created_at<=$4::text::timestamptz ORDER BY id LIMIT 100 FOR UPDATE")
        .bind(tenant)
        .bind(cursor)
        .bind(ceiling)
        .bind(snapshot)
        .fetch_all(tx)
        .await?
        .iter()
        .map(product)
        .collect())
}

/// Stores the fixed prices of one currency on a product and advances its revision.
pub async fn store_fixed_prices(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    currency: &str,
    prices: Value,
) -> Result<()> {
    sqlx::query("UPDATE products SET extra=jsonb_set(extra,'{currencyPrices}',coalesce(extra->'currencyPrices','{}') || jsonb_build_object($1::text,$2::jsonb)),revision=revision+1 WHERE tenant=$3 AND id=$4")
        .bind(currency)
        .bind(prices)
        .bind(tenant)
        .bind(product)
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn exists(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2)")
            .bind(tenant)
            .bind(product)
            .fetch_one(tx)
            .await?,
    )
}
