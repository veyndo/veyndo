//! Durable bounded embedding jobs; short claims and revision fences keep provider latency outside PostgreSQL.
use super::*;
use veyndo_intelligence::store::embedding_jobs as store;
pub(crate) fn model() -> String {
    env::var("EMBEDDING_MODEL").unwrap_or("qwen3-embedding:0.6b".into())
}
/// Interactive embedding reads share the same process and fleet admission as background indexing.
pub(crate) async fn query_embedding(a: &App, t: &str, text: &str) -> Option<Vec<f32>> {
    if veyndo::inference::budget::is_budgeted() {
        return None;
    }
    let _slot = a.inference_slots.clone().try_acquire_owned().ok()?;
    let _lease = crate::performance::cluster_lease::Lease::acquire(a, t, "embedding", 1)
        .await
        .ok()?;
    let (endpoint, key, _) = a.inference.connection("ollama").await.unwrap_or_default();
    if endpoint.is_empty() && env::var("EMBEDDING_BASE_URL").is_err() {
        return None;
    }
    let target = env::var("EMBEDDING_BASE_URL").unwrap_or(endpoint.clone());
    a.inference.admit_endpoint(&target).await.ok()?;
    knowledge::embedding(&a.http, &endpoint, key.as_deref(), &model(), text)
        .await
        .ok()
}
pub(crate) async fn enqueue(a: &App, t: &str, document: Option<&str>) -> Result<i64> {
    store::enqueue(a, t, document, &model()).await
}
pub(crate) async fn index_once(a: &App) -> Result<bool> {
    let model = model();
    let model_revision = super::generations::model_revision();
    let advanced = super::generations::advance(a, &model, model_revision).await?;
    let jobs = store::claim(a, &model, model_revision).await?;
    if jobs.is_empty() {
        return Ok(advanced);
    }
    let tenant: String = jobs[0].get("tenant");
    let sources = store::sources(a, &tenant, &jobs).await?;
    if sources.is_empty() {
        return Ok(true);
    }
    let (url, key, _) = a.inference.connection("ollama").await.unwrap_or_default();
    let url = env::var("EMBEDDING_BASE_URL").unwrap_or(url);
    a.inference.admit_endpoint(&url).await.map_err(bad)?;
    let key = env::var("EMBEDDING_API_KEY").ok().or(key);
    let texts = sources
        .iter()
        .map(|(_, text)| text.clone())
        .collect::<Vec<_>>();
    let slot = a.inference_slots.clone().try_acquire_owned().ok();
    let lease = if slot.is_some() {
        crate::performance::cluster_lease::Lease::acquire(a, &tenant, "embedding", 1)
            .await
            .ok()
    } else {
        None
    };
    let result = if lease.is_some() {
        knowledge::embeddings::batch(&a.http, &url, key.as_deref(), &model, &texts).await
    } else {
        Err("Embedding admission busy".into())
    };
    drop(lease);
    drop(slot);
    match result {
        Ok(vectors) => store::publish(a, &tenant, &model, model_revision, sources, vectors).await?,
        Err(error) => store::fail(a, &tenant, sources, &error).await?,
    }
    Ok(true)
}
