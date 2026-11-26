//! Actor-private workspace preferences; HTTP and MCP share CAS, bounds and the current membership.
use crate::store::{work_items, workspaces};
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::http::mfa::personal;
use veyndo_identity::merchant;

/// The caller as a personal merchant session with current second-factor admission.
async fn identity(a: &State, h: &RequestContext) -> Result<(String, String)> {
    let t = merchant(h)?;
    if !h.principal.personal_session {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal merchant session required".into(),
        ));
    }
    personal(a, h).await?;
    let user = h.principal.user.clone().ok_or(Error(
        StatusCode::FORBIDDEN,
        "A personal merchant session is required".into(),
    ))?;
    if h.principal.app.is_some() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal session required".into(),
        ));
    }
    Ok((t, user))
}
/// Reads or saves the personal workspace, or reads the work list.
pub async fn invoke(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let (t, user) = identity(a, h).await?;
    if name == "studio.workspace.work.read" {
        return work_items::load(a, h, &t, &user).await;
    }
    match name {
        "studio.workspace.read" => workspaces::read(a, &t, &user).await,
        "studio.workspace.save" => workspaces::save(a, &t, &user, v).await,
        _ => Err(bad("Unknown workspace operation")),
    }
}
/// The personal workspace.
pub async fn read(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "studio.workspace.read", &json!({})).await?,
    ))
}
/// Saves the personal workspace at its revision.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "studio.workspace.save", &v).await?))
}

/// The personal work list.
pub async fn work(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "studio.workspace.work.read", &json!({})).await?,
    ))
}
