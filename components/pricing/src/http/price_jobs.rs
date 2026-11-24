//! Merchant reads of price jobs.
use crate::ports::Pricing;
use crate::service::price_jobs::{NewJob, create as queue};
use crate::store::price_jobs;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
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

/// The JSON fields `create` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CreateInput {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
    #[serde(deserialize_with = "lenient")]
    overwrite: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

pub async fn create(
    State(p): State<Pricing>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: CreateInput = view(&v);
    permit(&h, "catalog.write")?;
    let tenant = merchant(&h)?;
    let job = NewJob {
        currency: input.currency.as_deref(),
        overwrite: input.overwrite == Some(true),
        revision: input.revision,
        actor: header(&h, "x-rac-user").unwrap_or("merchant"),
    };
    let id = queue(&p, &tenant, job).await?;
    Ok(Json(json!({"id":id,"state":"queued"})))
}
