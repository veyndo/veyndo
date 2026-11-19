//! Merchant reads of price jobs.
use crate::ports::Pricing;
use crate::service::price_jobs::{NewJob, create as queue};
use crate::store::price_jobs;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use veyndo_core::shop_scope::header;
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

pub async fn create(
    State(p): State<Pricing>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.write")?;
    let tenant = merchant(&h)?;
    let job = NewJob {
        currency: v["currency"].as_str(),
        overwrite: v["overwrite"] == true,
        revision: v["revision"].as_i64(),
        actor: header(&h, "x-rac-user").unwrap_or("merchant"),
    };
    let id = queue(&p, &tenant, job).await?;
    Ok(Json(json!({"id":id,"state":"queued"})))
}
