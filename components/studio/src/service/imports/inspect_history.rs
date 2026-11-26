//! Operational history comparison reads actual CRM/address/order/review values with paired current PII rights.
use super::{SourceRecord, relations};
use crate::store::imports as db;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_customers::service::address_entries::address_list;
use veyndo_customers::store::crm::address_plan;
use veyndo_identity::domain::credentials::email;
use veyndo_identity::{merchant, permit};
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

/// The native address book: its entries and default address IDs.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddressBook {
    #[serde(deserialize_with = "lenient")]
    elements: Option<Vec<Value>>,
    default_billing_address_id: Value,
    default_shipping_address_id: Value,
}
/// One address book entry.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AddressEntry {
    id: Value,
    address: Value,
}
/// Which defaults an imported address becomes.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AddressFlags {
    default_billing: Value,
    default_shipping: Value,
}
/// The fields of a historical source record (customer, order or review) the comparison reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HistorySource {
    #[serde(deserialize_with = "lenient")]
    first_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    last_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    created_at: Option<String>,
    #[serde(deserialize_with = "lenient")]
    lines: Option<Vec<Value>>,
    number: Value,
    #[serde(deserialize_with = "lenient")]
    total: Total,
    billing_address: Value,
    shipping_address: Value,
    #[serde(deserialize_with = "lenient")]
    product_source_id: Option<String>,
    source_author: Value,
    rating: Value,
    title: Value,
    text: Value,
}
/// An exact source amount.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Total {
    minor: Value,
    currency: Value,
    scale: Value,
}
/// A source order line's label and quantity.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Line {
    label: Value,
    quantity: Value,
}
/// The JSON fields `inspect` reads, typed as it reads them.
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
    let source: HistorySource = view(&r.data);
    permit(h, "customers.pii")?;
    let t = merchant(h)?;
    match r.kind.as_str() {
        "customers" => {
            permit(h, "customers.read")?;
            let row = db::customer(a, &t, id).await?;
            let mut actual = None;
            if let Some(row) = row {
                let email = row.email;
                let book: AddressBook = view(&address_list(a, &t, &email).await?);
                let elements = book
                    .elements
                    .ok_or(bad("Native address book unavailable"))?
                    .iter()
                    .map(view::<AddressEntry>)
                    .collect::<Vec<_>>();
                let default = |id: &Value| {
                    elements
                        .iter()
                        .find(|v| v.id == *id)
                        .map(|v| v.address.clone())
                        .unwrap_or(Value::Null)
                };
                let mut addresses = elements
                    .iter()
                    .map(|v| v.address.clone())
                    .collect::<Vec<_>>();
                addresses.sort_by_key(Value::to_string);
                actual = Some(
                    json!({"email":email,"profile":row.profile,"customerGroup":row.group,"sourceDigest":view::<CustomerAutomation>(&row.automation).import_source.fingerprint,"billingAddress":default(&book.default_billing_address_id),"shippingAddress":default(&book.default_shipping_address_id),"addresses":addresses}),
                );
            }
            let first = source.first_name.as_deref().unwrap_or("");
            let last = source.last_name.as_deref().unwrap_or("");
            let plan = address_plan(&r.data)?;
            let mut billing = Value::Null;
            let mut shipping = Value::Null;
            let mut addresses = vec![];
            for (address, flags) in plan {
                let address = json!(address);
                let flags: AddressFlags = view(&flags);
                if billing.is_null() || flags.default_billing == true {
                    billing = address.clone();
                }
                if shipping.is_null() || flags.default_shipping == true {
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
                source
                    .created_at
                    .as_deref()
                    .ok_or(bad("Source creation date required"))?,
            )
            .map_err(|_| bad("Invalid source creation date"))?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            let lines=source.lines.as_ref().ok_or(bad("Source order lines required"))?.iter().enumerate().map(|(index,line)|{let l:Line=view(line);json!({"id":format!("historical-{index}"),"label":l.label,"quantity":l.quantity,"type":"historical","sourceSnapshot":line})}).collect::<Vec<_>>();
            let total = &source.total;
            Ok((
                json!({"orderNumber":source.number,"state":"historical","sourceCreatedAt":created,"salesChannelId":input.channel_id,"money":{"minor":total.minor,"currency":{"code":total.currency,"scale":total.scale}},"cart":{"lineItems":lines,"price":{"totalMinor":total.minor,"currency":total.currency,"currencyScale":total.scale}},"historicalImport":{"snapshot":r.data,"fingerprint":hash(&json!({"record":r,"options":options}).to_string())},"billingAddress":source.billing_address,"shippingAddress":source.shipping_address,"payment":{"state":"historical","realMoneyCharged":false}}),
                actual,
            ))
        }
        "reviews" => {
            permit(h, "catalog.read")?;
            let actual = db::review(a, &t, id).await?;
            let created = chrono::DateTime::parse_from_rfc3339(
                source
                    .created_at
                    .as_deref()
                    .ok_or(bad("Source review date required"))?,
            )
            .map_err(|_| bad("Invalid review date"))?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            let product = relations::identity(
                r,
                &t,
                "products",
                source
                    .product_source_id
                    .as_deref()
                    .ok_or(bad("Source product required"))?,
            )?;
            Ok((
                json!({"productId":product,"sourceAuthor":source.source_author,"rating":source.rating,"title":source.title,"text":source.text,"createdAt":created,"verifiedPurchase":false,"sourceDigest":hash(&json!(r).to_string())}),
                actual,
            ))
        }
        _ => Err(bad("Unsupported history inspection")),
    }
}
