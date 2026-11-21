//! Report transport: one bounded report query per request.
use crate::service::reports::invoke;
use axum::Json;
use axum::extract::State as Shared;
use serde_json::Value;
use veyndo_core::{RequestContext, Result, State};

/// Runs the report the body describes.
pub async fn report(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, &v).await?))
}
