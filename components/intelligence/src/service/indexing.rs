//! Durable bounded embedding jobs; short claims and revision fences keep provider latency outside PostgreSQL.
use super::models::{Models, embedding_model as model};
use crate::knowledge;
use crate::store::embedding_generations::{advance, model_revision};
use crate::store::embedding_jobs as store;
use veyndo_cognition::inference::budget::is_budgeted;
use veyndo_core::store::lease::Lease;
use veyndo_core::{Result, State, bad};

/// Interactive embedding reads share the same process and fleet admission as background indexing.
pub async fn query_embedding(m: Models<'_>, t: &str, text: &str) -> Option<Vec<f32>> {
    if is_budgeted() {
        return None;
    }
    let _slot = m.slot().ok()?;
    let _lease = Lease::acquire(m.core, t, "embedding", 1).await.ok()?;
    let (endpoint, key, _) = m.inference.connection("ollama").await.unwrap_or_default();
    if endpoint.is_empty() && std::env::var("EMBEDDING_BASE_URL").is_err() {
        return None;
    }
    let target = std::env::var("EMBEDDING_BASE_URL").unwrap_or(endpoint.clone());
    m.inference.admit_endpoint(&target).await.ok()?;
    knowledge::embedding(&m.core.http, &endpoint, key.as_deref(), &model(), text)
        .await
        .ok()
}
/// Queues the shop's products, or one document, for embedding.
pub async fn enqueue(a: &State, t: &str, document: Option<&str>) -> Result<i64> {
    store::enqueue(a, t, document, &model()).await
}
/// One worker step: advance a model change, then embed one claimed batch.
pub async fn index_once(m: Models<'_>) -> Result<bool> {
    let model = model();
    let revision = model_revision();
    let advanced = advance(m.core, &model, revision).await?;
    let jobs = store::claim(m.core, &model, revision).await?;
    if jobs.is_empty() {
        return Ok(advanced);
    }
    let tenant = jobs[0].tenant.clone();
    let sources = store::sources(m.core, &tenant, &jobs).await?;
    if sources.is_empty() {
        return Ok(true);
    }
    let (url, key, _) = m.inference.connection("ollama").await.unwrap_or_default();
    let url = std::env::var("EMBEDDING_BASE_URL").unwrap_or(url);
    m.inference.admit_endpoint(&url).await.map_err(bad)?;
    let key = std::env::var("EMBEDDING_API_KEY").ok().or(key);
    let texts = sources
        .iter()
        .map(|(_, text)| text.clone())
        .collect::<Vec<_>>();
    let slot = m.slot().ok();
    let lease = if slot.is_some() {
        Lease::acquire(m.core, &tenant, "embedding", 1).await.ok()
    } else {
        None
    };
    let result = if lease.is_some() {
        knowledge::embeddings::batch(&m.core.http, &url, key.as_deref(), &model, &texts).await
    } else {
        Err("Embedding admission busy".into())
    };
    drop(lease);
    drop(slot);
    match result {
        Ok(vectors) => store::publish(m.core, &tenant, &model, revision, sources, vectors).await?,
        Err(error) => store::fail(m.core, &tenant, sources, &error).await?,
    }
    Ok(true)
}
