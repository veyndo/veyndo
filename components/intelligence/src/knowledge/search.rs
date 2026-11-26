//! Hybrid exact/lexical + dense RRF; hydrate current tenant rows and retrieve only connected evidence.
use super::PgPool;
use super::vectors;
use crate::store::search as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
pub async fn search(
    db: &PgPool,
    tenant: &str,
    query: &str,
    vector: Option<Vec<f32>>,
    model: &str,
) -> Result<Value, sqlx::Error> {
    let candidates = if let Some(vector) = vector {
        vectors::query("product", tenant, model, vector).await.ok()
    } else {
        None
    };
    let mode = if candidates.is_some() {
        "hybrid"
    } else {
        "lexical"
    };
    let hits = store::products(
        db,
        tenant,
        query,
        &json!(candidates.unwrap_or_default()),
        model,
    )
    .await?;
    // The application caller admits optional reranking through its shared inference budget.
    let reranked = false;
    let ids = hits
        .iter()
        .filter_map(|p| view::<Hit>(p).id)
        .collect::<Vec<_>>();
    Ok(
        json!({"mode":mode,"searchEngine":"Qdrant+PostgreSQL","fusion":"rrf","reranked":reranked,"hits":hits,"graph":neighborhood(db,tenant,&ids).await?}),
    )
}
/// An inference context is a bounded relevant subgraph, never a shop-wide graph dump.
pub async fn neighborhood(db: &PgPool, tenant: &str, ids: &[String]) -> Result<Value, sqlx::Error> {
    let edges = store::neighborhood(db, tenant, ids).await?;
    Ok(
        json!({"tenant":tenant,"engine":"PostgreSQL","edges":edges,"limit":48,"provenance":"curated or observed; association is not causal evidence"}),
    )
}

/// The product a search hit names.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Hit {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}
