//! MCP automation tools call the same tenant-bound handlers and validators as HTTP; no separate mutation semantics.
use super::{catalog, flow_policy, lifecycle, routes, rule_history};
use crate::{App, auth, merchant};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
/// Discovery admits exactly the domain scopes used by the invoked native handlers.
pub(crate) fn capability_visible(h: &RequestContext, name: &str) -> bool {
    let any = |scopes: &[&str]| scopes.iter().any(|scope| auth::allowed(h, scope));
    match name {
        "automation.compare_history" => rule_history::visible(h),
        "automation.catalog" | "automation.preview" => auth::allowed(h, "automation.read"),
        "automation.import" | "automation.validate_flow" => auth::allowed(h, "automation.manage"),
        "automation.list" | "automation.dependencies" => any(&["automation.read", "channels.read"]),
        "automation.save" | "automation.delete" => any(&["automation.manage", "channels.manage"]),
        _ => false,
    }
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    merchant(a, h)?;
    let Json(value) = match name {
        "automation.compare_history" => {
            rule_history::compare(State(a.clone()), h.clone(), Json(v.clone())).await?
        }
        "automation.catalog" => catalog::catalog(State(a.core.clone()), h.clone()).await?,
        "automation.list" => routes::list(State(a.clone()), h.clone()).await?,
        "automation.validate_flow" => {
            flow_policy::validate_draft(
                State(a.clone()),
                h.clone(),
                Path(v["id"].as_str().ok_or(bad("ID required"))?.into()),
                Json(v.clone()),
            )
            .await?
        }
        "automation.save" => {
            routes::save(
                State(a.clone()),
                h.clone(),
                Path((
                    v["kind"].as_str().ok_or(bad("Kind required"))?.into(),
                    v["id"].as_str().ok_or(bad("ID required"))?.into(),
                )),
                Json(v.clone()),
            )
            .await?
        }
        "automation.dependencies" => {
            lifecycle::dependencies(
                State(a.core.clone()),
                h.clone(),
                Path((
                    v["kind"].as_str().ok_or(bad("Kind required"))?.into(),
                    v["id"].as_str().ok_or(bad("ID required"))?.into(),
                )),
            )
            .await?
        }
        "automation.delete" => {
            lifecycle::delete(
                State(a.core.clone()),
                h.clone(),
                Path((
                    v["kind"].as_str().ok_or(bad("Kind required"))?.into(),
                    v["id"].as_str().ok_or(bad("ID required"))?.into(),
                )),
                Json(v.clone()),
            )
            .await?
        }
        "automation.preview" => {
            routes::preview(State(a.clone()), h.clone(), Json(v.clone())).await?
        }
        "automation.import" => catalog::import(h.clone(), Json(v["condition"].clone())).await?,
        _ => return Err(bad("Unknown automation capability")),
    };
    Ok(value)
}
