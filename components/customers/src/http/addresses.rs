//! Store API address book: independent customer sessions, never merchant credentials.
use crate::service::address_entries::{address_delete, address_list, address_save};
use crate::service::sessions::identify;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{session_digest, tenant};
use veyndo_core::{RequestContext, Result, State, bad};

/// The signed-in customer of the request and their shop.
async fn customer(a: &State, h: &RequestContext) -> Result<(String, String)> {
    let t = tenant(h)?;
    let email = identify(a, &t, &session_digest(h)?).await?;
    Ok((t, email))
}

/// `GET /store-api/account/addresses`.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let (t, e) = customer(&a, &h).await?;
    Ok(Json(address_list(&a, &t, &e).await?))
}

/// `POST /store-api/account/addresses`.
pub async fn create(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, e) = customer(&a, &h).await?;
    Ok(Json(address_save(&a, &t, &e, None, &v, &h).await?))
}

/// `PUT /store-api/account/addresses/{id}`.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (t, e) = customer(&a, &h).await?;
    Ok(Json(address_save(&a, &t, &e, Some(&id), &v, &h).await?))
}

/// The JSON fields `remove` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RemoveInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// `DELETE /store-api/account/addresses/{id}` at the expected revision.
pub async fn remove(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: RemoveInput = view(&v);
    let (t, e) = customer(&a, &h).await?;
    let revision = input.revision.ok_or(bad("revision required"))?;
    Ok(Json(address_delete(&a, &t, &e, &id, revision, &h).await?))
}
