//! Product rows as the shared product contract, and the bounded locked reads and writes price jobs use.
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, Row};
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;

/// A complete `products` row as the product contract reads it; checked queries select exactly these columns.
pub struct ProductRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub price: f64,
    pub tax_rate: f64,
    pub stock: i32,
    pub revision: i64,
    pub list_price: Option<f64>,
    pub regulation_price: Option<f64>,
    pub reference_price: Option<Value>,
    pub advanced_prices: Value,
    pub min_purchase: i32,
    pub purchase_steps: i32,
    pub max_purchase: Option<i32>,
    pub parent_id: Option<String>,
    pub options: Value,
    pub media: Value,
    pub properties: Value,
    pub delivery_days: i32,
    pub extra: Value,
    pub product_number: Option<String>,
}

impl From<ProductRow> for Product {
    fn from(r: ProductRow) -> Self {
        Product {
            product_number: r.product_number.unwrap_or_else(|| r.id.clone()),
            id: r.id,
            extra: r.extra,
            parent_id: r.parent_id,
            options: r.options,
            media: r.media,
            properties: r.properties,
            delivery_days: r.delivery_days,
            name: r.name,
            category: r.category,
            description: r.description,
            price: r.price,
            tax_rate: r.tax_rate,
            stock: r.stock,
            revision: r.revision,
            list_price: r.list_price,
            regulation_price: r.regulation_price,
            advanced_prices: serde_json::from_value(r.advanced_prices).unwrap_or_default(),
            min_purchase: r.min_purchase as u32,
            purchase_steps: r.purchase_steps as u32,
            max_purchase: r.max_purchase.map(|v| v as u32),
            reference_price: r
                .reference_price
                .and_then(|v| serde_json::from_value(v).ok()),
        }
    }
}

/// Hydrates the product contract from a row of the dynamic merchant list, which selects `p.*`.
pub fn product(r: &PgRow) -> Product {
    ProductRow {
        id: r.get("id"),
        name: r.get("name"),
        category: r.get("category"),
        description: r.get("description"),
        price: r.get("price"),
        tax_rate: r.get("tax_rate"),
        stock: r.get("stock"),
        revision: r.get("revision"),
        list_price: r.get("list_price"),
        regulation_price: r.get("regulation_price"),
        reference_price: r.get("reference_price"),
        advanced_prices: r.get("advanced_prices"),
        min_purchase: r.get("min_purchase"),
        purchase_steps: r.get("purchase_steps"),
        max_purchase: r.get("max_purchase"),
        parent_id: r.get("parent_id"),
        options: r.get("options"),
        media: r.get("media"),
        properties: r.get("properties"),
        delivery_days: r.get("delivery_days"),
        extra: r.get("extra"),
        product_number: r.get("product_number"),
    }
    .into()
}

/// The active main products of the template shop a new shop is seeded from, at most 101.
pub async fn prototype(a: &State, tenant: &str) -> Result<Vec<Product>> {
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND parent_id IS NULL AND active ORDER BY id LIMIT 101"#, tenant)
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(Product::from)
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
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id>$2 AND id<=$3 AND created_at<=$4::text::timestamptz ORDER BY id LIMIT 100 FOR UPDATE"#, tenant, cursor, ceiling, snapshot)
        .fetch_all(tx)
        .await?
        .into_iter()
        .map(Product::from)
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
    sqlx::query!(r#"UPDATE products SET extra=jsonb_set(extra,'{currencyPrices}',coalesce(extra->'currencyPrices','{}') || jsonb_build_object($1::text,$2::jsonb)),revision=revision+1 WHERE tenant=$3 AND id=$4"#, currency, prices, tenant, product)
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn exists(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        tenant,
        product
    )
    .fetch_one(tx)
    .await?)
}

/// The parent of a variant; `None` for a main product or an unknown ID.
pub async fn parent_of(a: &State, tenant: &str, product: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT parent_id FROM products WHERE tenant=$1 AND id=$2"#,
        tenant,
        product
    )
    .fetch_optional(&a.db)
    .await?
    .flatten())
}

/// A product and whether it is active (with an active parent) and not hidden in the channel.
pub async fn in_channel(
    a: &State,
    t: &str,
    id: &str,
    channel: &str,
) -> Result<Option<(Product, bool)>> {
    let Some(row) = sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price,tax_rate,stock,revision,list_price,regulation_price,reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id=$2"#, t, id)
        .fetch_optional(&a.db)
        .await?
    else {
        return Ok(None);
    };
    let visible:bool=sqlx::query_scalar!(r#"SELECT p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$3 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id)) AS "visible!" FROM products p WHERE p.tenant=$1 AND p.id=$2"#, t, id, channel).fetch_one(&a.db).await?;
    Ok(Some((row.into(), visible)))
}
