//! Knowledge-graph edges from private app sources to the products they reference.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

/// One `REFERENCES_PRODUCT` edge of an app source.
pub struct ProductReference {
    pub app: Option<String>,
    pub source_id: Option<String>,
    pub product_id: String,
}

/// The product references recorded for sources of the given apps.
pub async fn product_references<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    apps: &[String],
) -> Result<Vec<ProductReference>> {
    Ok(sqlx::query_as!(
        ProductReference,
        "SELECT data->>'app' AS app,data->>'sourceId' AS source_id,target_id AS product_id FROM knowledge_relations WHERE tenant=$1 AND kind='REFERENCES_PRODUCT' AND data->>'app'=ANY($2)",
        t,
        apps
    )
    .fetch_all(db)
    .await?)
}
