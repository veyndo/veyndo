//! Merchant category tree (list, save) and the public navigation.
use crate::ports::Catalog;
use crate::service::{categories, navigation as public};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result};

pub async fn list(State(c): State<Catalog>, h: RequestContext) -> Result<Json<Value>> {
    let (main, tree) = categories::list(&c, &h).await?;
    Ok(Json(json!({"mainLocale":main,"elements":tree})))
}

pub async fn save(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(categories::write(&c, &h, &id, v, false).await?))
}

/// The storefront navigation of the visitor's sales channel and language.
pub async fn navigation(State(c): State<Catalog>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(public::list(&c, &h).await?))
}
