//! Reads of image jobs.
use serde::Serialize;
use sqlx::FromRow;
use veyndo_core::{Result, State};

/// The newest image job of a product.
#[derive(Serialize, FromRow)]
pub struct LatestJob {
    #[serde(skip)]
    pub product_id: String,
    pub id: String,
    pub state: String,
}

pub async fn latest(a: &State, tenant: &str, products: &[String]) -> Result<Vec<LatestJob>> {
    Ok(sqlx::query_as::<_, LatestJob>("SELECT DISTINCT ON(product_id) product_id,id,state FROM media_jobs WHERE tenant=$1 AND product_id=ANY($2) ORDER BY product_id,created_at DESC,id DESC")
        .bind(tenant)
        .bind(products)
        .fetch_all(&a.db)
        .await?)
}
