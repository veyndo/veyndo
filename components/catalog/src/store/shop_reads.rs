//! Product reads for the shop component's operator views and settings guards: counts, tax class and price group references and recent reviews.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// Products of all shops outside `excluded`.
pub async fn count_outside(conn: &mut PgConnection, excluded: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM products WHERE NOT (tenant=ANY($1))"#,
        excluded
    )
    .fetch_one(conn)
    .await?)
}

/// Products per shop among `tenants`; shops without products are left out.
pub async fn count_by_shop(a: &State, tenants: &[String]) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM products WHERE tenant=ANY($1) GROUP BY tenant"#,
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}

/// Whether a product of the shop is assigned to a tax class outside `tax_ids`.
pub async fn tax_class_stranded(
    tx: &mut PgConnection,
    t: &str,
    tax_ids: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND extra->>'taxClassId' IS NOT NULL AND NOT(extra->>'taxClassId'=ANY($2))) AS "stranded!""#,
        t,
        tax_ids
    )
    .fetch_one(tx)
    .await?)
}

/// Whether an advanced price of the shop applies to one of the groups.
pub async fn group_prices(tx: &mut PgConnection, t: &str, groups: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products p CROSS JOIN LATERAL jsonb_array_elements(p.advanced_prices) x WHERE tenant=$1 AND x->>'rule_id'=ANY($2)) AS "price!""#, t, groups)
        .fetch_one(tx)
        .await?)
}

/// A product review as the settings view lists it.
pub struct RecentReview {
    pub id: String,
    pub product_id: String,
    pub author: String,
    pub rating: i32,
    pub title: String,
    pub content: String,
    pub verified: bool,
    pub approved: bool,
    pub demo: bool,
    pub source_import: Option<Value>,
    pub time: String,
}

/// The newest hundred reviews; imported ones only when `imports` is allowed.
pub async fn recent_reviews(a: &State, t: &str, imports: bool) -> Result<Vec<RecentReview>> {
    Ok(sqlx::query_as!(
        RecentReview,
        r#"SELECT id,product_id,author,rating,title,content,verified,approved,demo,source_import,created_at::text AS "time!" FROM product_reviews WHERE tenant=$1 AND (source_import IS NULL OR $2) ORDER BY created_at DESC LIMIT 100"#,
        t,
        imports
    )
    .fetch_all(&a.db)
    .await?)
}
