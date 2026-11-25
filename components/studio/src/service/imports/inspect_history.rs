//! Operational history comparison reads actual CRM/address/order/review values with paired current PII rights.
use super::{SourceRecord, relations};
use crate::store::imports as db;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_customers::service::address_entries::address_list;
use veyndo_customers::store::crm::address_plan;
use veyndo_identity::domain::credentials::email;
use veyndo_identity::{merchant, permit};
/// The JSON fields `inspect` reads, typed as it reads them.
/// The import source a customer's automation document records.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CustomerAutomation {
    import_source: ImportSource,
}
/// The fingerprint of the imported source record.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportSource {
    fingerprint: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InspectInput {
    customer_group: Value,
    channel_id: Value,
}

pub async fn inspect(
    a: &State,
    h: &RequestContext,
    r: &SourceRecord,
    options: &Value,
    id: &str,
) -> Result<(Value, Option<Value>)> {
    let input: InspectInput = view(options);
    permit(h, "customers.pii")?;
    let t = merchant(h)?;
    match r.kind.as_str() {
        "customers" => {
            permit(h, "customers.read")?;
            let row = db::customer(a, &t, id).await?;
            let mut actual = None;
            if let Some(row) = row {
                let email = row.email;
                let book = address_list(a, &t, &email).await?;
                let elements = book["elements"]
                    .as_array()
                    .ok_or(bad("Native address book unavailable"))?;
                let default = |field: &str| {
                    elements
                        .iter()
                        .find(|v| v["id"] == book[field])
                        .map(|v| v["address"].clone())
                        .unwrap_or(Value::Null)
                };
                let mut addresses = elements
                    .iter()
                    .map(|v| v["address"].clone())
                    .collect::<Vec<_>>();
                addresses.sort_by_key(Value::to_string);
                actual = Some(
                    json!({"email":email,"profile":row.profile,"customerGroup":row.group,"sourceDigest":view::<CustomerAutomation>(&row.automation).import_source.fingerprint,"billingAddress":default("defaultBillingAddressId"),"shippingAddress":default("defaultShippingAddressId"),"addresses":addresses}),
                );
            }
            let first = r.data["firstName"].as_str().unwrap_or("");
            let last = r.data["lastName"].as_str().unwrap_or("");
            let plan = address_plan(&r.data)?;
            let mut billing = Value::Null;
            let mut shipping = Value::Null;
            let mut addresses = vec![];
            for (address, flags) in plan {
                let address = json!(address);
                if billing.is_null() || flags["defaultBilling"] == true {
                    billing = address.clone();
                }
                if shipping.is_null() || flags["defaultShipping"] == true {
                    shipping = address.clone();
                }
                addresses.push(address);
            }
            addresses.sort_by_key(Value::to_string);
            Ok((
                json!({"email":email(&r.data)?,"profile":{"name":format!("{first} {last}"),"firstName":first,"lastName":last},"customerGroup":input.customer_group,"sourceDigest":hash(&json!(r).to_string()),"billingAddress":billing,"shippingAddress":shipping,"addresses":addresses}),
                actual,
            ))
        }
        "orders" => {
            permit(h, "orders.read")?;
            let actual = db::order(a, &t, id).await?;
            let created = chrono::DateTime::parse_from_rfc3339(
                r.data["createdAt"]
                    .as_str()
                    .ok_or(bad("Source creation date required"))?,
            )
            .map_err(|_| bad("Invalid source creation date"))?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            let lines=r.data["lines"].as_array().ok_or(bad("Source order lines required"))?.iter().enumerate().map(|(index,line)|json!({"id":format!("historical-{index}"),"label":line["label"],"quantity":line["quantity"],"type":"historical","sourceSnapshot":line})).collect::<Vec<_>>();
            Ok((
                json!({"orderNumber":r.data["number"],"state":"historical","sourceCreatedAt":created,"salesChannelId":input.channel_id,"money":{"minor":r.data["total"]["minor"],"currency":{"code":r.data["total"]["currency"],"scale":r.data["total"]["scale"]}},"cart":{"lineItems":lines,"price":{"totalMinor":r.data["total"]["minor"],"currency":r.data["total"]["currency"],"currencyScale":r.data["total"]["scale"]}},"historicalImport":{"snapshot":r.data,"fingerprint":hash(&json!({"record":r,"options":options}).to_string())},"billingAddress":r.data["billingAddress"],"shippingAddress":r.data["shippingAddress"],"payment":{"state":"historical","realMoneyCharged":false}}),
                actual,
            ))
        }
        "reviews" => {
            permit(h, "catalog.read")?;
            let actual = db::review(a, &t, id).await?;
            let created = chrono::DateTime::parse_from_rfc3339(
                r.data["createdAt"]
                    .as_str()
                    .ok_or(bad("Source review date required"))?,
            )
            .map_err(|_| bad("Invalid review date"))?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            let product = relations::identity(
                r,
                &t,
                "products",
                r.data["productSourceId"]
                    .as_str()
                    .ok_or(bad("Source product required"))?,
            )?;
            Ok((
                json!({"productId":product,"sourceAuthor":r.data["sourceAuthor"],"rating":r.data["rating"],"title":r.data["title"],"text":r.data["text"],"createdAt":created,"verifiedPurchase":false,"sourceDigest":hash(&json!(r).to_string())}),
                actual,
            ))
        }
        _ => Err(bad("Unsupported history inspection")),
    }
}
