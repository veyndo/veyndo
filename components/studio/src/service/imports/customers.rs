//! Historical CRM import reuses native validation and addresses, without credentials or ordinary new-customer events.
use super::SourceRecord;
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::{merchant, permit};
/// The reviewed fields of a source customer.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerSource {
    #[serde(deserialize_with = "lenient")]
    first_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    last_name: Option<String>,
    email: Value,
    billing_address: Value,
    shipping_address: Value,
    additional_addresses: Value,
}
/// The ID of an imported customer.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Created {
    id: Value,
}
/// The JSON fields `import` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportInput {
    customer_group: Value,
}

pub async fn import(
    a: &State,
    h: &RequestContext,
    r: &SourceRecord,
    options: &Value,
) -> Result<Value> {
    let input: ImportInput = view(options);
    permit(h, "customers.write")?;
    permit(h, "customers.pii")?;
    let t = merchant(h)?;
    if parent(a, &t).await?.is_some() {
        return Err(bad(
            "Historical customer data is not a configuration release unit",
        ));
    }
    let d: CustomerSource = view(&r.data);
    let first = d.first_name.as_deref().unwrap_or("");
    let last = d.last_name.as_deref().unwrap_or("");
    let address = &d.billing_address;
    let id = r.identity(&t);
    let mut payload = json!({"email":d.email,"profile":{"name":format!("{first} {last}"),"firstName":first,"lastName":last},"customerGroup":input.customer_group,"billingAddress":address,"requestKey":format!("migration:{id}")});
    if !d.shipping_address.is_null() {
        merge(
            &mut payload,
            json!({ "shippingAddress": d.shipping_address.clone() }),
        )?;
    }
    if !d.additional_addresses.is_null() {
        merge(
            &mut payload,
            json!({ "additionalAddresses": d.additional_addresses.clone() }),
        )?;
    }
    let provenance = json!({"source":r.source,"sourceShop":r.source_shop,"sourceId":r.source_id,"fingerprint":hash(&json!(r).to_string())});
    let Json(result) =
        veyndo_customers::http::merchant_crm::import(a, h, &id, payload, &provenance).await?;
    Ok(json!({"id":view::<Created>(&result).id,"created":true,"credentialMigrated":false}))
}
