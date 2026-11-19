//! Storefront review submission and merchant moderation.
use crate::ports::Catalog;
use crate::service::reviews as service;
use crate::store::reviews;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};
use veyndo_identity::merchant;

pub async fn submit(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(service::submit(&c, &h, &id, &v).await?))
}

pub async fn moderate(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let tenant = merchant(&h)?;
    let approved = v["approved"]
        .as_bool()
        .ok_or(bad("approved boolean required"))?;
    if !reviews::set_approved(&a, &tenant, &id, approved).await? {
        return Err(Error(StatusCode::NOT_FOUND, "Review not found".into()));
    }
    Ok(Json(json!({"approved":approved})))
}
