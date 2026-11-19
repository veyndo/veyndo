//! Applying reviewed translation drafts.
use crate::ports::Catalog;
use crate::service::translation_apply;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::Value;
use veyndo_core::{RequestContext, Result};

pub async fn apply(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(translation_apply::apply(&c, &h, &id, &v).await?))
}
