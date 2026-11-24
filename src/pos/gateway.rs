//! HTTP/MCP POS routes; the POS component owns the cashier gateway behind the register port.
use super::register_host::Register;
use crate::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
use veyndo_pos::service::register;
pub(super) async fn channels(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(register::channels(&a, &h).await?))
}
pub(super) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Path((channel, operation)): Path<(String, String)>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    if v.get("channelId").is_some_and(|id| id != &json!(channel)) {
        return Err(bad("POS channel context differs"));
    }
    v["channelId"] = json!(channel);
    Ok(Json(invoke(&a, &h, &operation, &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, op: &str, v: &Value) -> Result<Value> {
    Box::pin(register::invoke(&Register::new(a), a, h, op, v)).await
}
