//! HTTP proposal creation, approval and task listing.
use crate::{App, agent, apply, merchant};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn agent_apply(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    agent::assistant_access(&h)?;
    let t = merchant(&a, &h)?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    Ok(Json(apply(&a, &t, &id, &h).await?))
}
