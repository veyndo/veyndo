//! Prompt-generated declarative apps and native coding-agent handoff, never unsandboxed model code.
use crate::foundation::App;
use crate::{apps, auth};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::domain::{
    app_generation, design_context, generation_brief, generation_edits,
};
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
pub(crate) mod agent_connection;
mod builds;
mod generation;
mod routes;
pub(crate) use routes::router;

pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    auth::permit(
        h,
        if name == "developer.builds" {
            "developer.read"
        } else {
            "developer.manage"
        },
    )?;
    // Cross-environment package work uses the same trusted owner for HTTP and MCP.
    veyndo::tenant_scope::scoped(
        veyndo::tenant_scope::Scope::System,
        invoke_admitted(a, h, name, v),
    )
    .await
}
async fn invoke_admitted(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    Ok(match name {
        "developer.archive" => {
            veyndo_app_platform::service::app_builds::archive(&a.core, h, v).await?
        }
        "developer.builds" => routes::list(State(a.clone()), h.clone()).await?.0,
        "developer.import" => {
            routes::import(State(a.clone()), h.clone(), Json(v.clone()))
                .await?
                .0
        }
        "developer.stage" => {
            let id = v["buildId"].as_str().ok_or(bad("Build ID required"))?;
            veyndo_app_platform::service::app_builds::stage(
                &apps::service_limits::platform(a),
                h,
                id,
                &json!({"approve":v["approve"],"digest":v["digest"]}),
            )
            .await?
        }
        "developer.task" => {
            routes::task(State(a.clone()), h.clone(), Json(v.clone()))
                .await?
                .0
        }
        _ => return Err(bad("Unknown developer tool")),
    })
}
