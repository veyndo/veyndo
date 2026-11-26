//! Atomic restoration of the owning customer's address aggregate; geography, references and immutable order snapshots remain guarded.
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};
use veyndo_pricing::customer::Address;

/// The JSON fields `restore` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RestoreInput {
    addresses: Value,
    #[serde(deserialize_with = "lenient")]
    default_billing_address_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    default_shipping_address_id: Option<String>,
}

/// One address of a historical address book.
#[derive(Deserialize, Default)]
#[serde(default)]
struct HistoricalAddress {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    address: Value,
}

/// `admit` decides whether the shop still accepts an address: its country, and its region within that country.
pub async fn restore(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    state: &Value,
    admit: &(dyn Fn(&Address) -> Result<()> + Sync),
) -> Result<()> {
    let input: RestoreInput = view(state);
    let entries = input
        .addresses
        .as_array()
        .filter(|v| v.len() <= 100)
        .ok_or(bad("Invalid historical address book"))?;
    let mut ids = Vec::new();
    for entry in entries {
        let entry: HistoricalAddress = view(entry);
        let id = entry
            .id
            .as_deref()
            .ok_or(bad("Historical address identity required"))?;
        if id.len() > 100 || ids.contains(&id.to_string()) {
            return Err(bad("Invalid historical address identity"));
        }
        let mut address: Address =
            serde_json::from_value(entry.address).map_err(|_| bad("Invalid historical address"))?;
        address.validate()?;
        admit(&address)?;
        let owner = sqlx::query_scalar!(
            "SELECT email FROM customer_addresses WHERE tenant=$1 AND id=$2 FOR UPDATE",
            t,
            id
        )
        .fetch_optional(&mut *conn)
        .await?;
        if owner.as_ref().is_some_and(|owner| owner != email) {
            return Err(bad("Historical address belongs to another customer"));
        }
        sqlx::query!("INSERT INTO customer_addresses(tenant,email,id,data) VALUES($1,$2,$3,$4) ON CONFLICT(tenant,id) DO UPDATE SET data=EXCLUDED.data,revision=customer_addresses.revision+1,updated_at=now() WHERE customer_addresses.email=EXCLUDED.email", t, email, id, json!(address)).execute(&mut *conn).await?;
        ids.push(id.to_string());
    }
    sqlx::query!(
        "DELETE FROM customer_addresses WHERE tenant=$1 AND email=$2 AND NOT(id=ANY($3))",
        t,
        email,
        &ids
    )
    .execute(&mut *conn)
    .await?;
    let bill = input.default_billing_address_id.as_deref();
    let ship = input.default_shipping_address_id.as_deref();
    if [bill, ship]
        .into_iter()
        .flatten()
        .any(|id| !ids.iter().any(|v| v == id))
    {
        return Err(bad("Historical default address is missing"));
    }
    sqlx::query!("UPDATE customers SET default_billing_address_id=$3,default_shipping_address_id=$4,revision=revision+1 WHERE tenant=$1 AND email=$2", t, email, bill, ship).execute(conn).await?;
    Ok(())
}
