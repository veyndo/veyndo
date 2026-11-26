//! Grid filter planning lives in the studio component; this handler lends it the server's model context.
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_studio::service::grid_plan::visible;
pub(crate) async fn plan(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_studio::service::grid_plan::plan(a.models(), &h, v).await?,
    ))
}
