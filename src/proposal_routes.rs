//! HTTP proposal creation, approval and task listing.
use crate::agent;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::proposal_apply::apply;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;

/// The JSON fields `agent_apply` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AgentApplyInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
}

pub(crate) async fn agent_apply(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: AgentApplyInput = view(&v);
    agent::assistant_access(&h)?;
    let t = merchant(&a, &h)?;
    if input.approve != Some(true) {
        return Err(bad("Explicit approve=true required"));
    }
    Ok(Json(apply(&a, &t, &id, &h).await?))
}
