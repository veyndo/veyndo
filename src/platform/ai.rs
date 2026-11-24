//! Platform inference routes; the intelligence component owns the configuration, sealing and audit.
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::service::platform_ai;
pub(super) async fn get(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(platform_ai::get(a.models(), &h).await?))
}
pub(super) async fn save(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(platform_ai::save(a.models(), &h, &v).await?))
}
