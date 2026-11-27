//! The address book of a customer: bounded, revision-checked, with defaults that are never left empty.
use crate::store::addresses::{self, Entry};
use crate::types::events::{AddressChanged, AddressDeleted};
use serde::Serialize;
use serde_json::json;
use sqlx::PgConnection;
use veyndo_core::events;
use veyndo_core::{Error, Result, State, StatusCode, bad, conflict, uid};
use veyndo_pricing::customer::Address;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub elements: Vec<Entry>,
    pub default_billing_address_id: Option<String>,
    pub default_shipping_address_id: Option<String>,
    pub customer_revision: i64,
}

pub async fn list(a: &State, t: &str, email: &str) -> Result<Book> {
    let elements = addresses::list(a, t, email).await?;
    let defaults = addresses::defaults(a, t, email)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Customer not found".into()))?;
    Ok(Book {
        elements,
        default_billing_address_id: defaults.default_billing_address_id,
        default_shipping_address_id: defaults.default_shipping_address_id,
        customer_revision: defaults.revision,
    })
}

/// What a save asks for besides the address itself.
pub struct Save<'a> {
    /// The address to replace; `None` adds a new one.
    pub id: Option<&'a str>,
    /// The revision the replaced address was read at.
    pub revision: Option<i64>,
    pub default_billing: bool,
    pub default_shipping: bool,
    /// Imports restore existing data and announce nothing.
    pub announce: bool,
}

/// Stores a checked address and returns its ID. The caller owns the transaction and its history attribution.
pub async fn save(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    request: Save<'_>,
    address: &Address,
) -> Result<String> {
    if !addresses::lock_customer(conn, t, email).await? {
        return Err(Error(StatusCode::NOT_FOUND, "Customer not found".into()));
    }
    let id = request.id.map(str::to_owned).unwrap_or_else(uid);
    if request.id.is_some() {
        let revision = request.revision.ok_or(bad("Address revision required"))?;
        if !addresses::update(conn, t, email, &id, revision, json!(address)).await? {
            return Err(conflict("Address changed or unavailable"));
        }
    } else {
        if addresses::count(conn, t, email).await? >= 100 {
            return Err(bad("Address book limit reached"));
        }
        addresses::insert(conn, t, email, &id, json!(address)).await?;
    }
    addresses::assign_defaults(
        conn,
        t,
        email,
        &id,
        request.default_billing,
        request.default_shipping,
    )
    .await?;
    if request.announce {
        let changed = AddressChanged {
            email,
            address_id: &id,
        };
        events::emit(&mut *conn, t, &changed).await?;
    }
    Ok(id)
}

/// Deletes an address at the revision it was read at. The caller owns the transaction and its history attribution.
pub async fn delete(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
    revision: i64,
) -> Result<()> {
    if !addresses::lock_customer(conn, t, email).await? {
        return Err(Error(StatusCode::NOT_FOUND, "Customer not found".into()));
    }
    if !addresses::delete(conn, t, email, id, revision).await? {
        return Err(conflict("Address changed or unavailable"));
    }
    addresses::touch_customer(conn, t, email).await?;
    let deleted = AddressDeleted {
        email,
        address_id: id,
    };
    events::emit(conn, t, &deleted).await
}

/// A stored address of the customer, checked again and share-locked for the rest of the transaction.
pub async fn get(conn: &mut PgConnection, t: &str, email: &str, id: &str) -> Result<Address> {
    let data = addresses::get_shared(conn, t, email, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Address not found".into()))?;
    let mut address: Address =
        serde_json::from_value(data).map_err(|_| bad("Invalid stored address"))?;
    address.validate()?;
    Ok(address)
}
