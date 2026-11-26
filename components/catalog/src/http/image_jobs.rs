//! Image jobs of a product: provider status, queueing, review and publication.
use crate::ports::Catalog;
use crate::service::image_jobs;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::Value;
use veyndo_core::{RequestContext, Result};

pub async fn provider(State(c): State<Catalog>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(image_jobs::provider(&c, &h).await?))
}

pub async fn enqueue(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(product): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(image_jobs::enqueue(&c, &h, &product, &v).await?))
}

pub async fn list(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(product): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(image_jobs::list(&c, &h, &product).await?))
}

pub async fn detail(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(image_jobs::detail(&c, &h, &id).await?))
}

pub async fn apply(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(image_jobs::apply(&c, &h, &id, &v).await?))
}
