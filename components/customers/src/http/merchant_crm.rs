//! Merchant CRM writes: customer creation and the address book, with the same ownership and revision checks as the Store API.
use crate::service::address_entries::{address_delete, address_list, address_save};
use crate::store::crm;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};
use veyndo_marketing::service::conditions::validate_condition;

/// Creates a customer from the merchant CRM; it never issues credentials.
pub async fn create(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    create_inner(a, h, v, None, None).await
}
/// Creates an imported customer with a stable ID and its provenance.
pub async fn import(
    a: &State,
    h: &RequestContext,
    id: &str,
    v: Value,
    source: &Value,
) -> Result<Json<Value>> {
    create_inner(a.clone(), h.clone(), v, Some(id), Some(source)).await
}
/// The shared merchant creation path.
async fn create_inner(
    a: State,
    h: RequestContext,
    v: Value,
    stable_id: Option<&str>,
    source: Option<&Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    Ok(Json(
        crm::create(&a, &h, &t, v, stable_id, source, &validate_condition).await?,
    ))
}

/// The addresses of a customer.
pub async fn addresses(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(email): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.read")?;
    permit(&h, "customers.pii")?;
    Ok(Json(address_list(&a, &t, &email).await?))
}
/// Adds an address to a customer.
pub async fn create_address(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(email): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    Ok(Json(address_save(&a, &t, &email, None, &v, &h).await?))
}
/// Saves a customer address at its revision.
pub async fn save_address(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((email, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    Ok(Json(address_save(&a, &t, &email, Some(&id), &v, &h).await?))
}
/// The JSON fields `remove_address` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RemoveAddressInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// Deletes a customer address at its revision.
pub async fn remove_address(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((email, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: RemoveAddressInput = view(&v);
    let t = merchant(&h)?;
    permit(&h, "customers.write")?;
    permit(&h, "customers.pii")?;
    Ok(Json(
        address_delete(
            &a,
            &t,
            &email,
            &id,
            input.revision.ok_or(bad("revision required"))?,
            &h,
        )
        .await?,
    ))
}
