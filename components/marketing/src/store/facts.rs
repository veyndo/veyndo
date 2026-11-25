//! Assemble private server-owned rule context once per quote/event; never publish customer facts in cart responses.
use crate::domain::line_facts::line;
use crate::ports::records;
use crate::store::customer_facts::customer;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::domain::family::inherited_extra;
use veyndo_catalog::store::rule_products;
use veyndo_core::Result;
use veyndo_core::document::{Raw, lenient, merge, view};
use veyndo_shop::types::cart::{StoredCart, selection};
/// The quote fields rule facts are assembled from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FactsQuote {
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    price: Raw<QuoteCurrency>,
    shipping_costs: Value,
    #[serde(deserialize_with = "lenient")]
    discounts: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    discount_total: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct QuoteCurrency {
    currency: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ItemReference {
    #[serde(deserialize_with = "lenient")]
    referenced_id: Option<String>,
}

pub async fn rule_facts(conn: &mut sqlx::PgConnection, c: &StoredCart, q: &Value) -> Result<Value> {
    let ids = c
        .data
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect::<Vec<_>>();
    let rows = rule_products::by_ids(conn, &c.tenant, &ids).await?;
    let parent_ids = rows
        .iter()
        .filter_map(|p| p.parent_id.clone())
        .collect::<Vec<_>>();
    let parents = rule_products::by_ids(conn, &c.tenant, &parent_ids).await?;
    let mut lines = vec![];
    let quote: FactsQuote = view(q);
    for item in quote.line_items.iter().flatten() {
        let referenced = view::<ItemReference>(item).referenced_id;
        if let Some(r) = rows
            .iter()
            .find(|r| Some(r.id.as_str()) == referenced.as_deref())
        {
            let mut p = r.clone();
            if let Some(parent) = parents
                .iter()
                .find(|r| Some(r.id.as_str()) == p.parent_id.as_deref())
            {
                p.extra = inherited_extra(&parent.extra, &p.extra);
            }
            lines.push(line(&p, item));
        }
    }
    let selected = selection(&c.data);
    let totals = lines.iter().map(view::<LineTotals>).collect::<Vec<_>>();
    let mut facts = json!({"lines":lines,"price":quote.price.raw,"shippingCosts":quote.shipping_costs,"checkout":selected,"salesChannel":c.data.sales_channel,"currency":quote.price.currency,"language":if c.data.locale.is_empty(){"en-GB"}else{&c.data.locale},"adminSource":false,"goodsCount":lines.len(),"lineCount":lines.len(),"goodsPrice":totals.iter().map(|l|l.total_price.as_f64().unwrap_or(0.)).sum::<f64>(),"weight":totals.iter().map(|l|l.weight.as_f64().unwrap_or(0.)*l.quantity.as_f64().unwrap_or(0.)).sum::<f64>(),"volume":totals.iter().map(|l|l.volume.as_f64().unwrap_or(0.)*l.quantity.as_f64().unwrap_or(0.)).sum::<f64>(),"hasDeliveryFreeItem":totals.iter().any(|l|l.shipping_free==true),"promotionCount":quote.discounts.as_ref().map(Vec::len).unwrap_or(0),"promotionValue":quote.discount_total.unwrap_or(0.)});
    let mut addresses = serde_json::Map::new();
    for (key, address) in [
        ("billing", selected.billing_address),
        ("shipping", selected.address),
    ] {
        let value = match address {
            Some(a) => {
                let mut v = json!(a);
                let postal = view::<PostalCode>(&v).postal_code;
                merge(&mut v, json!({ "zipcode": postal }))?;
                v
            }
            None => {
                json!({"country":if key=="shipping"{json!(selected.country.clone())}else{Value::Null},"city":null,"street":null,"zipcode":null,"countryStateId":null})
            }
        };
        addresses.insert(key.into(), value);
    }
    merge(&mut facts, addresses)?;
    let customer = customer(conn, c, &facts).await?;
    let now = sqlx::query_scalar!(
        r#"SELECT to_char(now() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "now!""#
    )
    .fetch_one(&mut *conn)
    .await?;
    merge(
        &mut facts,
        json!({ "customer": customer, "currentTime": now }),
    )?;
    Ok(facts)
}
/// The order fields order rules read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OrderFacts {
    state: Value,
    #[serde(deserialize_with = "lenient")]
    automation: AutomationFacts,
    affiliate_code: Value,
    campaign_code: Value,
    #[serde(deserialize_with = "lenient")]
    created_by_admin: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    deliveries: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    payment: StateOnly,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AutomationFacts {
    #[serde(deserialize_with = "lenient")]
    tags: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    custom_fields: Option<serde_json::Map<String, Value>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StateOnly {
    state: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DeliveryFacts {
    state: Value,
    #[serde(deserialize_with = "lenient")]
    tracking_code: Option<String>,
}

pub async fn order_facts(
    conn: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    q: &Value,
    facts: &mut Value,
) -> Result<()> {
    let receipts = records()?.receipt_kinds(conn, t, id).await?;
    let q: OrderFacts = view(q);
    let deliveries = q
        .deliveries
        .iter()
        .flatten()
        .map(view::<DeliveryFacts>)
        .collect::<Vec<_>>();
    merge(
        facts,
        json!({"order":{"state":q.state,"tags":q.automation.tags.unwrap_or_default(),"customFields":q.automation.custom_fields.unwrap_or_default(),"affiliateCode":q.affiliate_code,"campaignCode":q.campaign_code,"createdByAdmin":q.created_by_admin.unwrap_or(false),"documentTypes":receipts,"sentDocumentTypes":[],"deliveryStates":deliveries.iter().map(|d|d.state.clone()).collect::<Vec<_>>(),"paymentStates":[q.payment.state],"trackingSet":deliveries.iter().any(|d|d.tracking_code.as_deref().is_some_and(|s|!s.is_empty()))}}),
    )?;
    Ok(())
}

/// The per-line amounts cart totals are summed from.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LineTotals {
    total_price: Value,
    weight: Value,
    quantity: Value,
    volume: Value,
    shipping_free: Value,
}

/// The postal code of an address, which rules read as `zipcode`.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PostalCode {
    postal_code: Value,
}
