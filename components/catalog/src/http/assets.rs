//! Merchant product files: list, upload and publish.
use crate::http::ingestion;
use crate::ports::Catalog;
use crate::service::assets as service;
use crate::store::assets;
use axum::Json;
use axum::extract::{Multipart, Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::{merchant, permit};

pub async fn list(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let tenant = merchant(&h)?;
    permit(&h, "catalog.read")?;
    Ok(Json(
        json!({"elements": assets::of_product(&a, &tenant, &id).await?}),
    ))
}

pub async fn upload(
    State(c): State<Catalog>,
    h: RequestContext,
    Path(product): Path<String>,
    multipart: Multipart,
) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "catalog")?;
    let upload = ingestion::parse(multipart, false).await?;
    service::persist(&c, &h, &product, upload).await.map(Json)
}

/// The JSON fields `publish` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PublishInput {
    #[serde(deserialize_with = "lenient")]
    public: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    digest: Option<String>,
}

pub async fn publish(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: PublishInput = view(&v);
    let tenant = merchant(&h)?;
    permit(&h, "catalog")?;
    let public = input.public.ok_or(bad("public required"))?;
    let digest = input.digest.as_deref().ok_or(bad("digest required"))?;
    if !assets::set_public(&a, &tenant, &id, digest, public).await? {
        return Err(conflict("Asset digest changed or unavailable"));
    }
    Ok(Json(json!({"saved":true})))
}
