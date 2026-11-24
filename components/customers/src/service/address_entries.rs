//! Entries of the address book: country admission against the shop settings and history attribution, around the address book persistence.
use crate::service::address_book;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::history_context::{context, customer_context};
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_pricing::customer::Address;
use veyndo_shipping::domain::geography::validate_address;
use veyndo_shop::service::read_cache::settings;

/// The customer's address book.
pub async fn address_list(a: &State, t: &str, email: &str) -> Result<Value> {
    Ok(json!(address_book::list(a, t, email).await?))
}
/// The JSON fields `address_save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddressSaveInput {
    address: Value,
}

/// Validates an address against the shop countries and saves it, attributed to the merchant or the customer.
pub async fn address_save(
    a: &State,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    h: &RequestContext,
) -> Result<Value> {
    let input: AddressSaveInput = view(v);
    let mut address: Address =
        serde_json::from_value(input.address.clone()).map_err(|_| bad("Invalid address"))?;
    address.validate()?;
    let (s, _) = settings(a, t, "default").await?;
    validate_address(
        &address.country,
        &address.country_state_id,
        &s.country_definitions,
    )?;
    if !s.countries.contains(&address.country) {
        return Err(bad("Address country unavailable"));
    }
    let mut tx = a.db.begin().await?;
    if header(h, "x-rac-user").is_some() {
        context(&mut tx, h, "merchant").await?;
    } else {
        customer_context(&mut tx, email).await?;
    }
    let result = address_save_conn(&mut tx, t, email, id, v, &address).await?;
    tx.commit().await?;
    Ok(result)
}
/// Saves an already validated address on the caller's connection.
pub async fn address_save_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    address: &Address,
) -> Result<Value> {
    address_store_conn(conn, t, email, id, v, address).await
}
/// The JSON fields `address_store_conn` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddressStoreConnInput {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    default_billing: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    default_shipping: Option<bool>,
}

/// Persists the address through the address book and announces the change.
async fn address_store_conn(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: Option<&str>,
    v: &Value,
    address: &Address,
) -> Result<Value> {
    let input: AddressStoreConnInput = view(v);
    let address_id = address_book::save(
        conn,
        t,
        email,
        address_book::Save {
            id,
            revision: input.revision,
            default_billing: input.default_billing.unwrap_or(false),
            default_shipping: input.default_shipping.unwrap_or(false),
            announce: true,
        },
        address,
    )
    .await?;
    Ok(json!({"id":address_id,"saved":true}))
}
/// Deletes an address at its revision, attributed to the merchant or the customer.
pub async fn address_delete(
    a: &State,
    t: &str,
    email: &str,
    id: &str,
    revision: i64,
    h: &RequestContext,
) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    if header(h, "x-rac-user").is_some() {
        context(&mut tx, h, "merchant").await?;
    } else {
        customer_context(&mut tx, email).await?;
    }
    address_book::delete(&mut tx, t, email, id, revision).await?;
    tx.commit().await?;
    Ok(json!({"deleted":true}))
}
/// One address of the customer, read on the caller's connection.
pub async fn address_get(
    conn: &mut sqlx::PgConnection,
    t: &str,
    email: &str,
    id: &str,
) -> Result<Address> {
    address_book::get(conn, t, email, id).await
}
