//! The recent reviews and orders shown beside the merchant settings: reviews from the catalog store, orders through the dependants port.
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
    Ok(
        veyndo_catalog::store::shop_reads::recent_reviews(a, t, imports)
            .await?
            .into_iter()
            .map(|r| Review {
                id: r.id,
                product_id: r.product_id,
                author: r.author,
                rating: r.rating,
                title: r.title,
                content: r.content,
                verified: r.verified,
                approved: r.approved,
                demo: r.demo,
                source_import: r.source_import,
                time: r.time,
            })
            .collect(),
    )
}

/// The newest thirty order documents without their cart token.
pub async fn orders(t: &str) -> Result<Vec<Value>> {
    crate::ports::dependants()?.recent_orders(t).await
}
