//! Model provider listing and product retrieval over the server's model context; conversations and knowledge handlers live in the intelligence component.
use super::*;
pub(crate) use veyndo_identity::assistant_access;
pub(crate) use veyndo_intelligence::http::agents::{
    chat as merchant_chat, conversation, conversation_list as conversations, knowledge_graph,
    knowledge_status, reindex,
};
pub(crate) use veyndo_intelligence::service::models::choice;

pub(super) async fn model_providers(
    State(a): State<App>,
    h: RequestContext,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    if ![
        "assistant.use",
        "developer.read",
        "knowledge.read",
        "ai.manage",
    ]
    .iter()
    .any(|s| auth::allowed(&h, s))
    {
        auth::permit(&h, "assistant.use")?;
    }
    Ok(Json(a.inference.public_providers().await.map_err(bad)?))
}
pub(super) async fn retrieve(a: &App, t: &str, q: &str) -> Result<Value> {
    veyndo_intelligence::service::retrieval::retrieve(a.models(), t, q).await
}
pub(super) async fn semantic_search(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        retrieve(&a, &tenant(&h)?, v["query"].as_str().unwrap_or("")).await?,
    ))
}
