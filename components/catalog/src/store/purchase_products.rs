//! The product facts a purchase and the product detail page read from the catalog, on the caller's connection: product locks, channel visibility and selection, and published reviews.
use crate::store::exact::Exact;
use crate::store::products::ProductRow;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;
use veyndo_pricing::catalog::Product;

/// The distinct parents of the given products.
pub async fn parents(tx: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT parent_id AS "parent_id!" FROM products WHERE tenant=$1 AND id=ANY($2) AND parent_id IS NOT NULL"#,
        tenant,
        ids
    )
    .fetch_all(tx)
    .await?)
}

/// A product row-locked for a purchase, with whether it is active.
pub struct LockedProduct {
    pub active: bool,
    pub product: Product,
}

/// The products, row-locked in id order, as the shared product contract reads them.
pub async fn lock(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[String],
) -> Result<Vec<LockedProduct>> {
    let rows = sqlx::query!(r#"SELECT id,name,category,description,price_decimal AS "price!: Exact",tax_rate_decimal AS "tax_rate!: Exact",stock,revision,list_price_decimal AS "list_price: Exact",regulation_price_decimal AS "regulation_price: Exact",reference_price,advanced_prices,min_purchase,purchase_steps,max_purchase,parent_id,options,media,properties,delivery_days,extra,product_number,active FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE"#, tenant, ids)
        .fetch_all(tx)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| LockedProduct {
            active: r.active,
            product: ProductRow {
                id: r.id,
                name: r.name,
                category: r.category,
                description: r.description,
                price: r.price,
                tax_rate: r.tax_rate,
                stock: r.stock,
                revision: r.revision,
                list_price: r.list_price,
                regulation_price: r.regulation_price,
                reference_price: r.reference_price,
                advanced_prices: r.advanced_prices,
                min_purchase: r.min_purchase,
                purchase_steps: r.purchase_steps,
                max_purchase: r.max_purchase,
                parent_id: r.parent_id,
                options: r.options,
                media: r.media,
                properties: r.properties,
                delivery_days: r.delivery_days,
                extra: r.extra,
                product_number: r.product_number,
            }
            .into(),
        })
        .collect())
}

/// Any of the products is hidden in the channel.
pub async fn hidden(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[String],
    channel: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM product_channel_visibility WHERE tenant=$1 AND product_id=ANY($2) AND channel_id=$3 AND NOT visible) AS "hidden!""#,
        tenant,
        ids,
        channel
    )
    .fetch_one(tx)
    .await?)
}

/// Any of the products is neither selected nor a variant of a selected product.
pub async fn outside_selection(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[String],
    selected: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=ANY($2) AND NOT (id=ANY($3) OR coalesce(parent_id=ANY($3),false))) AS "exists!""#, tenant, ids, selected)
        .fetch_one(tx)
        .await?)
}

/// A published review.
pub struct Review {
    pub id: String,
    pub author: String,
    pub rating: i32,
    pub title: String,
    pub content: String,
    pub verified: bool,
    pub demo: bool,
    pub imported: bool,
    pub time: String,
}

/// The family's latest 50 approved reviews (the product and its variants), newest first.
pub async fn published_reviews<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    family: &str,
) -> Result<Vec<Review>> {
    Ok(sqlx::query_as!(
        Review,
        r#"SELECT r.id,r.author,r.rating,r.title,r.content,r.verified,r.demo,(r.source_import IS NOT NULL) AS "imported!",r.created_at::text AS "time!" FROM product_reviews r WHERE r.tenant=$1 AND r.approved AND (r.product_id=$2 OR EXISTS(SELECT 1 FROM products p WHERE p.tenant=r.tenant AND p.id=r.product_id AND p.parent_id=$2)) ORDER BY r.created_at DESC LIMIT 50"#,
        tenant,
        family
    )
    .fetch_all(db)
    .await?)
}

/// The count and average rating of the family's approved reviews.
pub struct ReviewSummary {
    pub count: i64,
    pub rating: f64,
}

/// The aggregate over all approved reviews of the family, independent of pagination.
pub async fn review_summary<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    family: &str,
) -> Result<ReviewSummary> {
    Ok(sqlx::query_as!(
        ReviewSummary,
        r#"SELECT count(*) AS "count!",coalesce(avg(r.rating),0)::double precision AS "rating!" FROM product_reviews r WHERE r.tenant=$1 AND r.approved AND (r.product_id=$2 OR EXISTS(SELECT 1 FROM products p WHERE p.tenant=r.tenant AND p.id=r.product_id AND p.parent_id=$2))"#,
        tenant,
        family
    )
    .fetch_one(db)
    .await?)
}
