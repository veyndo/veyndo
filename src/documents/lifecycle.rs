//! Source detail, editing and publication live in the intelligence component; these entries keep the server-side signatures.
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::http::sources;
pub(crate) async fn detail(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    sources::detail(State(a.core.clone()), h, Path(id)).await
}
pub(crate) async fn edit(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    sources::edit(State(a.core.clone()), h, Path(id), Json(v)).await
}
pub(crate) async fn lifecycle(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    sources::lifecycle(State(a.core.clone()), h, Path(id), Json(v)).await
}
