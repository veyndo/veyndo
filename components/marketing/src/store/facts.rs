//! Assemble private server-owned rule context once per quote/event; never publish customer facts in cart responses.
use crate::domain::line_facts::line;
use crate::store::customer_facts::customer;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::domain::family::inherited_extra;
use veyndo_catalog::store::products::product;
use veyndo_core::Result;
use veyndo_core::document::{lenient, view};
use veyndo_shop::types::cart::{StoredCart, selection};
pub async fn rule_facts(conn: &mut sqlx::PgConnection, c: &StoredCart, q: &Value) -> Result<Value> {
    let ids = c
        .data
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect::<Vec<_>>();
    let rows = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2)")
        .bind(&c.tenant)
        .bind(&ids)
        .fetch_all(&mut *conn)
        .await?;
    let parent_ids = rows
        .iter()
        .filter_map(|r| r.get::<Option<String>, _>("parent_id"))
        .collect::<Vec<_>>();
    let parents = sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2)")
        .bind(&c.tenant)
        .bind(parent_ids)
        .fetch_all(&mut *conn)
        .await?;
    let mut lines = vec![];
    for item in q["lineItems"].as_array().into_iter().flatten() {
        if let Some(r) = rows
            .iter()
            .find(|r| Some(r.get::<String, _>("id").as_str()) == item["referencedId"].as_str())
        {
            let mut p = product(r);
            if let Some(parent) = parents
                .iter()
                .find(|r| Some(r.get::<String, _>("id").as_str()) == p.parent_id.as_deref())
            {
                p.extra = inherited_extra(&product(parent).extra, &p.extra);
            }
            lines.push(line(&p, item));
        }
    }
    let selected = selection(&c.data);
    let mut facts = json!({"lines":lines,"price":q["price"],"shippingCosts":q["shippingCosts"],"checkout":selected,"salesChannel":c.data.sales_channel,"currency":q["price"]["currency"],"language":if c.data.locale.is_empty(){"en-GB"}else{&c.data.locale},"adminSource":false,"goodsCount":lines.len(),"lineCount":lines.len(),"goodsPrice":lines.iter().map(|l|l["totalPrice"].as_f64().unwrap_or(0.)).sum::<f64>(),"weight":lines.iter().map(|l|l["weight"].as_f64().unwrap_or(0.)*l["quantity"].as_f64().unwrap_or(0.)).sum::<f64>(),"volume":lines.iter().map(|l|l["volume"].as_f64().unwrap_or(0.)*l["quantity"].as_f64().unwrap_or(0.)).sum::<f64>(),"hasDeliveryFreeItem":lines.iter().any(|l|l["shippingFree"]==true),"promotionCount":q["discounts"].as_array().map(Vec::len).unwrap_or(0),"promotionValue":q["discountTotal"].as_f64().unwrap_or(0.)});
    for (key, address) in [
        ("billing", selected.billing_address),
        ("shipping", selected.address),
    ] {
        facts[key]=address.map(|a|{let mut v=json!(a);v["zipcode"]=v["postalCode"].clone();v}).unwrap_or(json!({"country":if key=="shipping"{json!(selected.country.clone())}else{Value::Null},"city":null,"street":null,"zipcode":null,"countryStateId":null}));
    }
    facts["customer"] = customer(conn, c, &facts).await?;
    facts["currentTime"] = json!(
        sqlx::query_scalar::<_, String>(
            "SELECT to_char(now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')"
        )
        .fetch_one(&mut *conn)
        .await?
    );
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
    let receipts = sqlx::query_scalar::<_, String>(
        "SELECT kind FROM order_receipts WHERE tenant=$1 AND order_id=$2",
    )
    .bind(t)
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let q: OrderFacts = view(q);
    let deliveries = q
        .deliveries
        .iter()
        .flatten()
        .map(view::<DeliveryFacts>)
        .collect::<Vec<_>>();
    facts["order"] = json!({"state":q.state,"tags":q.automation.tags.unwrap_or_default(),"customFields":q.automation.custom_fields.unwrap_or_default(),"affiliateCode":q.affiliate_code,"campaignCode":q.campaign_code,"createdByAdmin":q.created_by_admin.unwrap_or(false),"documentTypes":receipts,"sentDocumentTypes":[],"deliveryStates":deliveries.iter().map(|d|d.state.clone()).collect::<Vec<_>>(),"paymentStates":[q.payment.state],"trackingSet":deliveries.iter().any(|d|d.tracking_code.as_deref().is_some_and(|s|!s.is_empty()))});
    Ok(())
}
