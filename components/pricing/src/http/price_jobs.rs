//! Merchant reads of price jobs.
use crate::store::price_jobs;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, StatusCode};
use veyndo_identity::{merchant, permit};

pub async fn list(State(a): State<veyndo_core::State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let tenant = merchant(&h)?;
    Ok(Json(
        json!({"jobs": price_jobs::recent(&a, &tenant).await?}),
    ))
}

pub async fn detail(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let tenant = merchant(&h)?;
    price_jobs::find(&a, &tenant, &id)
        .await?
        .map(|job| Json(json!(job)))
        .ok_or(Error(StatusCode::NOT_FOUND, "Price job not found".into()))
}
