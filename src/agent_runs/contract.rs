//! HTTP/MCP share revision-bound run controls and safe progress; scenarios, agent controls and run operations are dispatched by name.
use super::scenarios;
use crate::{App, SecureRoutes};
use axum::Router;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::http::agents;
pub(crate) use veyndo_intelligence::service::agent_runs::CAPABILITIES;
use veyndo_intelligence::service::agent_runs::{operate, schema as run_schema};
pub(crate) fn schema(name: &str) -> Option<Value> {
    scenarios::schema(name)
        .or_else(|| super::controls::schema(name))
        .or_else(|| run_schema(name))
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .merge(scenarios::router())
        .merge(super::controls::router())
        .secure_route(
            "/api/agent/runs",
            &[("POST", "assistant.use")],
            post(agents::create),
        )
        .secure_route(
            "/api/agent/runs/{id}",
            &[("GET", "assistant.use")],
            get(agents::status),
        )
        .secure_route(
            "/api/agent/runs/{id}/cancel",
            &[("POST", "assistant.use")],
            post(agents::cancel),
        )
        .secure_route(
            "/api/agent/runs/{id}/resume",
            &[("POST", "assistant.use")],
            post(agents::resume),
        )
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    if scenarios::schema(name).is_some() {
        return Box::pin(scenarios::invoke(a, h, name, v)).await;
    }
    if super::controls::schema(name).is_some() {
        return super::controls::invoke(a, h, name, v).await;
    }
    operate(a, h, name, v).await
}
