//! Model provider listing and product retrieval over the server's model context; conversations and knowledge handlers live in the intelligence component.
use super::auth;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
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
    Ok(Json(
        a.models.inference.public_providers().await.map_err(bad)?,
    ))
}
pub(super) async fn retrieve(a: &App, t: &str, q: &str) -> Result<Value> {
    veyndo_intelligence::service::retrieval::retrieve(a.models(), t, q).await
}
/// The JSON fields `semantic_search` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SemanticSearchInput {
    #[serde(deserialize_with = "lenient")]
    query: Option<String>,
}

pub(super) async fn semantic_search(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SemanticSearchInput = view(&v);
    Ok(Json(
        retrieve(&a, &tenant(&h)?, input.query.as_deref().unwrap_or("")).await?,
    ))
}
