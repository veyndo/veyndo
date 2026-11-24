//! Fictional shopper cohorts live in the intelligence component; the server registers their routes and lends the agent port.
use crate::agent_host::Host;
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_intelligence::service::scenarios::{CAPABILITIES, schema, visible};
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::scenarios::invoke(&Host(a), h, name, v).await
}
pub(super) async fn execute(
    a: &App,
    t: &str,
    id: &str,
    h: &RequestContext,
    request: &Value,
) -> Result<Value> {
    veyndo_intelligence::service::scenarios::execute(&Host(a), t, id, h, request).await
}
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/agent/scenarios",
            &[("POST", "assistant.use"), ("GET", "assistant.use")],
            post(create).get(list),
        )
        .secure_route(
            "/api/agent/scenario-stock",
            &[("POST", "catalog.write")],
            post(stock),
        )
        .secure_route(
            "/api/agent/scenario-readiness",
            &[("GET", "assistant.use")],
            get(readiness),
        )
        .secure_route(
            "/api/agent/scenario-probes",
            &[("POST", "assistant.use"), ("GET", "assistant.use")],
            post(probe).get(attacks),
        )
        .secure_route(
            "/api/agent/scenarios/{id}",
            &[("GET", "assistant.use")],
            get(status),
        )
}
async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.scenario.create", &v).await?))
}
async fn readiness(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.scenario.readiness", &json!({})).await?,
    ))
}
async fn stock(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "merchant.scenario.stock", &v).await?))
}
async fn attacks(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.scenario.attacks", &json!({})).await?,
    ))
}
async fn probe(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.scenario.probe", &json!({})).await?,
    ))
}
async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.scenario.list", &json!({})).await?,
    ))
}
async fn status(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "merchant.scenario.status", &json!({"id":id})).await?,
    ))
}
