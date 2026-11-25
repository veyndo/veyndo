//! The recent reviews and orders shown beside the merchant settings. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A product review as the settings view lists it.
pub struct Review {
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
pub async fn reviews(a: &State, t: &str, imports: bool) -> Result<Vec<Review>> {
    Ok(sqlx::query_as!(
        Review,
        r#"SELECT id,product_id,author,rating,title,content,verified,approved,demo,source_import,created_at::text AS "time!" FROM product_reviews WHERE tenant=$1 AND (source_import IS NULL OR $2) ORDER BY created_at DESC LIMIT 100"#,
        t,
        imports
    )
    .fetch_all(&a.db)
    .await?)
}

/// The newest thirty order documents without their cart token.
pub async fn orders(a: &State, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT data #- '{cart,token}' AS "data!" FROM orders WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30"#,
        t
    )
    .fetch_all(&a.db)
    .await?)
}
