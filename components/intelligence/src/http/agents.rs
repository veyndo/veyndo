//! HTTP handlers of durable agent runs, agent controls and merchant conversations; the server registers them with their route rights.
use crate::service::{agent_controls, agent_runs, conversations, retrieval};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_core::document::merge;
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{merchant, permit};

/// Queues a durable planning run.
pub async fn create(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        agent_runs::operate(&a, &h, "merchant.run.create", &v).await?,
    ))
}
/// One saved run with current rights.
pub async fn status(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        agent_runs::operate(&a, &h, "merchant.run.status", &json!({"id":id})).await?,
    ))
}
/// Why a saved run acted, with the source references the reader may see.
pub async fn explanation(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        agent_runs::operate(&a, &h, "merchant.run.explain", &json!({"id":id})).await?,
    ))
}
/// Cancels a saved run at its expected revision.
pub async fn cancel(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    merge(&mut v, json!({ "id": id }))?;
    Ok(Json(
        agent_runs::operate(&a, &h, "merchant.run.cancel", &v).await?,
    ))
}
/// Explicitly resumes a failed or uncertain run.
pub async fn resume(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    merge(&mut v, json!({ "id": id }))?;
    Ok(Json(
        agent_runs::operate(&a, &h, "merchant.run.resume", &v).await?,
    ))
}
/// The live shop's agent stop state.
pub async fn control_status(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        agent_controls::invoke(&a, &h, "merchant.agents.status", &json!({})).await?,
    ))
}
/// Stops or re-enables the shop's agents at the current revision.
pub async fn control(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        agent_controls::invoke(&a, &h, "merchant.agents.control", &v).await?,
    ))
}
/// The caller's conversations.
pub async fn conversation_list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(conversations::list(&a, &h).await?))
}
/// One conversation with its messages and latest run.
pub async fn conversation(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(conversations::conversation(&a, &h, &id).await?))
}
/// One merchant chat turn.
pub async fn chat(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(conversations::chat(&a, &h, &v).await?))
}
/// The shop's index status.
pub async fn knowledge_status(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    Ok(Json(retrieval::knowledge_status(&a, &t).await?))
}
/// Queues the shop's products for embedding.
pub async fn reindex(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "knowledge.write")?;
    let t = merchant(&h)?;
    let queued = crate::service::indexing::enqueue(&a, &t, None).await?;
    Ok(Json(
        json!({"queued":queued,"indexed":0,"model":crate::service::models::embedding_model(),"engine":"Qdrant","asynchronous":true}),
    ))
}
/// The tenant's knowledge graph.
pub async fn knowledge_graph(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        crate::knowledge::graph(&a.db, &veyndo_core::shop_scope::tenant(&h)?).await?,
    ))
}
