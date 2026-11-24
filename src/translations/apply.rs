//! Entry point for capabilities; applying reviewed drafts is owned by the catalog component.
use crate::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn apply(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    veyndo_catalog::http::translations::apply(
        State(axum::extract::FromRef::from_ref(&a)),
        h,
        Path(id),
        Json(v),
    )
    .await
}
