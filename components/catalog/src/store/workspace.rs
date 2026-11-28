//! Catalogue reads and writes of the merchant workspace: product rows for proposals and the editor, reviewed price/stock changes at the aggregate or content clock, existence checks, products out of stock and translation jobs with reviewable items.
use super::products::ProductRow;
use crate::store::exact::Exact;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::{Result, State};
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::Decimal;

/// Up to four products without stock, by ID.
pub async fn out_of_stock(a: &State, t: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM products WHERE tenant=$1 AND stock<=0 ORDER BY id LIMIT 4",
        t
    )
    .fetch_all(&a.db)
    .await?)
}

/// The three newest translation jobs with items ready for review.
pub async fn reviewable_translations(a: &State, t: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT id FROM translation_jobs WHERE tenant=$1 AND EXISTS(SELECT 1 FROM translation_items i WHERE i.tenant=translation_jobs.tenant AND i.job_id=translation_jobs.id AND i.status='ready') ORDER BY created_at DESC LIMIT 3", t).fetch_all(&a.db).await?)
}

/// Up to 100 of the requested products.
pub async fn requested(a: &State, t: &str, ids: &[String]) -> Result<Vec<Product>> {
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price_decimal AS "price!: Exact",tax_rate_decimal AS "tax_rate!: Exact",stock,revision,list_price_decimal AS "list_price: Exact",regulation_price_decimal AS "regulation_price: Exact",reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id LIMIT 100"#, t, ids)
        .fetch_all(&a.db)
        .await?
        .into_iter()
        .map(Product::from)
        .collect())
}

/// The given products, locked in ID order.
pub async fn lock(c: &mut PgConnection, t: &str, ids: &[String]) -> Result<Vec<Product>> {
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price_decimal AS "price!: Exact",tax_rate_decimal AS "tax_rate!: Exact",stock,revision,list_price_decimal AS "list_price: Exact",regulation_price_decimal AS "regulation_price: Exact",reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE"#, t, ids)
        .fetch_all(&mut *c)
        .await?
        .into_iter()
        .map(Product::from)
        .collect())
}

/// One product inside a transaction.
pub async fn get(c: &mut PgConnection, t: &str, id: &str) -> Result<Product> {
    Ok(sqlx::query_as!(ProductRow, r#"SELECT id,name,category,description,price_decimal AS "price!: Exact",tax_rate_decimal AS "tax_rate!: Exact",stock,revision,list_price_decimal AS "list_price: Exact",regulation_price_decimal AS "regulation_price: Exact",reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number FROM products WHERE tenant=$1 AND id=$2"#, t, id)
        .fetch_one(&mut *c)
        .await?
        .into())
}

/// The columns of a product the merchant editor shows.
pub struct EditorProduct {
    pub name: String,
    pub description: String,
    pub active: bool,
    pub product_number: Option<String>,
    pub parent_id: Option<String>,
    pub options: Value,
    pub revision: i64,
    pub content_revision: i64,
    pub extra: Value,
    pub price: Exact,
    pub tax_rate: Exact,
    pub stock: i32,
    pub min_purchase: i32,
    pub purchase_steps: i32,
    pub max_purchase: Option<i32>,
    pub delivery_days: i32,
    pub list_price: Option<Exact>,
    pub regulation_price: Option<Exact>,
    pub reference_price: Option<Value>,
    pub advanced_prices: Value,
    pub media: Value,
    pub properties: Value,
}

impl EditorProduct {
    /// The commerce fields in the shape the editor reads (the same document as `product_fields::editable`).
    pub fn commerce(&self) -> Value {
        json!({"price":self.price,"taxRate":self.tax_rate,"stock":self.stock,"minPurchase":self.min_purchase,"purchaseSteps":self.purchase_steps,"maxPurchase":self.max_purchase,"deliveryDays":self.delivery_days,"listPrice":self.list_price,"regulationPrice":self.regulation_price,"referencePrice":self.reference_price,"advancedPrices":self.advanced_prices,"media":self.media,"properties":self.properties})
    }
}

/// A product as the merchant editor reads it.
pub async fn editor_product(a: &State, t: &str, id: &str) -> Result<Option<EditorProduct>> {
    Ok(sqlx::query_as!(EditorProduct, "SELECT name,description,active,product_number,parent_id,options,revision,content_revision,extra,price_decimal AS \"price!: Exact\",tax_rate_decimal AS \"tax_rate!: Exact\",stock,min_purchase,purchase_steps,max_purchase,delivery_days,list_price_decimal AS \"list_price: Exact\",regulation_price_decimal AS \"regulation_price: Exact\",reference_price,advanced_prices,media,properties FROM products WHERE tenant=$1 AND id=$2", t, id)
        .fetch_optional(&a.db)
        .await?)
}

/// A product's translation in each given locale as `(locale, name, description)`; missing ones are `None`.
pub async fn translations(
    a: &State,
    t: &str,
    id: &str,
    locales: &[String],
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    Ok(sqlx::query!(r#"SELECT l.locale,p.name AS "name?",p.description AS "description?" FROM languages l LEFT JOIN product_translations p ON p.language_id=l.id AND p.tenant=$1 AND p.product_id=$2 WHERE l.locale=ANY($3)"#, t, id, locales).fetch_all(&a.db).await?.into_iter().map(|r| (r.locale, r.name, r.description)).collect())
}

/// Applies a reviewed price/stock change at its expected revision; returns the rows changed.
pub async fn apply_change(
    c: &mut PgConnection,
    t: &str,
    (product_id, expected_revision): (&str, i64),
    price: Option<Decimal>,
    stock: Option<i32>,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET price_decimal=COALESCE($1,price_decimal),stock=COALESCE($2,stock),revision=revision+1 WHERE tenant=$3 AND id=$4 AND revision=$5", price.map(Exact) as Option<Exact>, stock, t, product_id, expected_revision).execute(&mut *c).await?.rows_affected())
}

/// A product's two concurrency clocks: the aggregate revision (stock included) and the content revision.
pub struct Clocks {
    pub id: String,
    pub revision: i64,
    pub content_revision: i64,
}

/// The clocks of the given products, read inside the caller's transaction (locked rows stay locked).
pub async fn clocks(c: &mut PgConnection, t: &str, ids: &[String]) -> Result<Vec<Clocks>> {
    Ok(sqlx::query_as!(
        Clocks,
        "SELECT id,revision,content_revision FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id",
        t,
        ids
    )
    .fetch_all(&mut *c)
    .await?)
}

/// Sets a reviewed price at its expected content clock and keeps the current stock, as a content-only manual edit does; returns the rows changed.
pub async fn apply_price(
    c: &mut PgConnection,
    t: &str,
    (product_id, expected_content_revision): (&str, i64),
    price: Decimal,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE products SET price_decimal=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND content_revision=$4", Exact(price) as Exact, t, product_id, expected_content_revision).execute(&mut *c).await?.rows_affected())
}

/// Whether a product exists.
pub async fn exists(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&a.db)
    .await?)
}

/// Whether a category exists.
pub async fn category_exists(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM categories WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(&a.db)
    .await?)
}
