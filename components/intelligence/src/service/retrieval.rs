//! Bounded retrieval before inference: product search with optional reranking, source search and the index status.
use super::indexing::query_embedding;
use super::models::{Models, embedding_model};
use crate::knowledge;
use crate::store::context_products::localized_products;
use crate::store::documents as sources;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_cognition::inference::budget::is_budgeted;
use veyndo_core::store::lease::Lease;
use veyndo_core::{Result, State, bad};
use veyndo_pricing::domain::catalog::Product;
use veyndo_shop::service::read_cache::config;

/// Hybrid product search; the vector half needs indexed products and the reranker a free slot.
pub async fn retrieve(m: Models<'_>, t: &str, q: &str) -> Result<Value> {
    if q.is_empty() || q.len() > 2000 {
        return Err(bad("Query must contain 1..2000 characters"));
    }
    let model = embedding_model();
    let indexed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM semantic_products WHERE tenant=$1 AND embedding_model=$2)",
    )
    .bind(t)
    .bind(&model)
    .fetch_one(&m.core.db)
    .await?;
    let vector = if indexed {
        query_embedding(
            m,
            t,
            &format!("Instruct: Retrieve suitable commerce products.\nQuery: {q}"),
        )
        .await
    } else {
        None
    };
    let mut result = knowledge::search(&m.core.db, t, q, vector, &model).await?;
    if !is_budgeted()
        && let Ok(reranker) = std::env::var("RERANKER_URL")
        && m.inference.admit_endpoint(&reranker).await.is_ok()
        && let Ok(_slot) = m.slot()
        && let Ok(_lease) = Lease::acquire(m.core, t, "reranker", 1).await
        && let Some(hits) = result["hits"].as_array_mut()
    {
        let reranked = knowledge::rerank::apply(t, q, hits, &m.core.db).await;
        result["reranked"] = json!(reranked);
    }
    result["embeddingModel"] = json!(model);
    result["hasIndexedProducts"] = json!(indexed);
    Ok(result)
}
/// One retrieval shared by the localized products of a prompt and the advisor's relevant graph.
pub async fn context_retrieval(
    m: Models<'_>,
    t: &str,
    chain: &[String],
    query: &str,
) -> Result<(Vec<Product>, Value)> {
    let search = retrieve(m, t, query).await?;
    let ids = search["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    Ok((
        localized_products(m.core, t, chain, &ids, true).await?,
        search,
    ))
}
/// Where a source search looks.
pub struct SourceQuery<'a> {
    /// Only sources of this product, if set.
    pub product: Option<&'a str>,
    /// The searched text.
    pub query: &'a str,
    /// Only published sources.
    pub public: bool,
    /// The requested content language.
    pub locale: &'a str,
    /// Whether the vector half may run.
    pub semantic: bool,
}
/// Lexical and, when indexed, vector search over knowledge sources.
pub async fn search_in(m: Models<'_>, t: &str, q: SourceQuery<'_>) -> Result<Value> {
    let (settings, _) = config(m.core, t).await?;
    let model = embedding_model();
    let indexed = sources::indexed(m.core, t, &model, q.public, q.product).await?;
    let vector = if indexed && q.semantic {
        query_embedding(m, t, q.query).await
    } else {
        None
    };
    sources::search(
        m.core,
        t,
        sources::Search {
            product: q.product,
            query: q.query,
            public: q.public,
            locale: q.locale,
            main_locale: &settings.main_locale,
            model: &model,
        },
        vector,
    )
    .await
}
/// The shop's index progress, its error counts and the first indexed products.
pub async fn knowledge_status(a: &State, t: &str) -> Result<Value> {
    let rows = sqlx::query("SELECT product_id,embedding_model,content_hash,vector_digest FROM semantic_products WHERE tenant=$1 ORDER BY product_id LIMIT 200")
        .bind(t).fetch_all(&a.db).await?;
    let status = sqlx::query(
        "SELECT indexed_products,pending_jobs FROM knowledge_index_status WHERE tenant=$1",
    )
    .bind(t)
    .fetch_optional(&a.db)
    .await?;
    let errors:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('code',e.key,'count',e.value) FROM knowledge_index_status s CROSS JOIN LATERAL jsonb_each(s.errors) e WHERE tenant=$1 ORDER BY e.key LIMIT 16").bind(t).fetch_all(&a.db).await?;
    Ok(
        json!({"indexedProducts":status.as_ref().map(|r|r.get::<i64,_>("indexed_products")).unwrap_or(0),"pending":status.as_ref().map(|r|r.get::<i64,_>("pending_jobs")).unwrap_or(0),"errors":errors,"documentsLimit":200,"documents":rows.iter().map(|r|json!({"productId":r.get::<String,_>("product_id"),"model":r.get::<String,_>("embedding_model"),"contentHash":r.get::<String,_>("content_hash"),"vectorDigest":r.get::<String,_>("vector_digest")})).collect::<Vec<_>>()}),
    )
}
