//! Model provider listing and product retrieval over the server's model context; conversations and knowledge handlers live in the intelligence component.
use super::{App, auth, merchant};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
pub(crate) use veyndo_identity::assistant_access;

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
