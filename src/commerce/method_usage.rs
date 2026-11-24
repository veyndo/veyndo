//! Authoritative deletion guards for methods; the reference counts are owned by the shipping component.
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn method_dependencies(
    State(a): State<App>,
    h: RequestContext,
    path: Path<(String, String)>,
) -> Result<Json<Value>> {
    let Json(found) =
        veyndo_shipping::http::method_usage::dependencies(State(a.core.clone()), h, path).await?;
    Ok(Json(json!(found)))
}
