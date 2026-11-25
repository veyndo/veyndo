//! Historical CRM import reuses native validation and addresses, without credentials or ordinary new-customer events.
use super::SourceRecord;
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::{merchant, permit};
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
    let d = &r.data;
    let first = d["firstName"].as_str().unwrap_or("");
    let last = d["lastName"].as_str().unwrap_or("");
    let address = &d["billingAddress"];
    let id = r.identity(&t);
    let mut payload = json!({"email":d["email"],"profile":{"name":format!("{first} {last}"),"firstName":first,"lastName":last},"customerGroup":input.customer_group,"billingAddress":address,"requestKey":format!("migration:{id}")});
    if !d["shippingAddress"].is_null() {
        payload["shippingAddress"] = d["shippingAddress"].clone();
    }
    if !d["additionalAddresses"].is_null() {
        payload["additionalAddresses"] = d["additionalAddresses"].clone();
    }
    let provenance = json!({"source":r.source,"sourceShop":r.source_shop,"sourceId":r.source_id,"fingerprint":hash(&json!(r).to_string())});
    let Json(result) =
        veyndo_customers::http::merchant_crm::import(a, h, &id, payload, &provenance).await?;
    Ok(json!({"id":result["id"],"created":true,"credentialMigrated":false}))
}
