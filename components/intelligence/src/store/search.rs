//! Product retrieval in storage: hybrid lexical/dense candidates hydrated from current tenant rows, and the bounded product neighborhood. Checked against the schema at compile time.
use serde_json::Value;
use veyndo_runtime::scoped_pool::ScopedPool;

/// Hydrated product hits for a query, fused with the dense candidates of `model`.
pub async fn products(
    db: &ScopedPool,
    tenant: &str,
    query: &str,
    candidates: &Value,
    model: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_file_scalar!("src/store/search.sql", tenant, query, candidates, model)
        .fetch_all(db)
        .await
}

/// The two-hop relation neighborhood of the products.
pub async fn neighborhood(
    db: &ScopedPool,
    tenant: &str,
    ids: &[String],
) -> Result<Vec<Value>, sqlx::Error> {
    sqlx::query_file_scalar!("src/store/neighborhood.sql", tenant, ids)
        .fetch_all(db)
        .await
}
