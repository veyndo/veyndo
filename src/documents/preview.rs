//! Retrieval preview route; the intelligence component owns the audience and locale rules.
use super::{Knowledge, product_questions};
use crate::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn preview(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::preview(&Knowledge(&a), &h, &v).await?,
    ))
}
