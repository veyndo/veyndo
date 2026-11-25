//! Published reviews of a product family as product facts of the detail page. Columns checked at compile time.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

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
pub async fn published<'e>(
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
pub struct Summary {
    pub count: i64,
    pub rating: f64,
}

/// The aggregate over all approved reviews of the family, independent of pagination.
pub async fn summary<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    family: &str,
) -> Result<Summary> {
    Ok(sqlx::query_as!(
        Summary,
        r#"SELECT count(*) AS "count!",coalesce(avg(r.rating),0)::double precision AS "rating!" FROM product_reviews r WHERE r.tenant=$1 AND r.approved AND (r.product_id=$2 OR EXISTS(SELECT 1 FROM products p WHERE p.tenant=r.tenant AND p.id=r.product_id AND p.parent_id=$2))"#,
        tenant,
        family
    )
    .fetch_one(db)
    .await?)
}
