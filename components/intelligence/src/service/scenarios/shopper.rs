//! A closed shopper adapter uses native carts and exact reviewed totals; no model-supplied authority or payment mode.
use crate::ports::AgentHost;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_shipping::domain::geography::catalogue;
use veyndo_shop::service::read_cache::settings as scoped_config;
pub fn headers(t: &str, request: &Value, token: Option<&str>) -> Result<RequestContext> {
    let mut h = RequestContext::new();
    for (key, value) in [
        ("x-tenant", t),
        (
            "x-commerce-locale",
            request["locale"].as_str().unwrap_or("en-GB"),
        ),
        (
            "sw-sales-channel-id",
            request["channel"].as_str().unwrap_or("default"),
        ),
        (
            "x-commerce-currency",
            request["scenario"]["currency"].as_str().unwrap_or(""),
        ),
    ] {
        h.insert(
            key,
            value.parse().map_err(|_| bad("Invalid scenario context"))?,
        );
    }
    if let Some(token) = token {
        h.insert(
            "sw-context-token",
            token.parse().map_err(|_| bad("Invalid private cart"))?,
        );
    }
    Ok(h)
}
pub async fn call<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    Box::pin(host.read(h, name, v)).await
}
/// The JSON fields `observation` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ObservationInput {
    revision: Value,
    status: Value,
    price: Value,
    selection_needs_confirmation: Value,
    line_items: Value,
}

pub fn observation(v: &Value) -> Value {
    let args: ObservationInput = view(v);
    // Positive projection: nested customer/configuration payloads and capabilities
    // never enter model input or reports, including after a completed checkout.
    json!({"revision":args.revision,"status":args.status,"price":args.price,"selectionNeedsConfirmation":args.selection_needs_confirmation,
        "items":args.line_items.as_array().map(|items|items.iter().map(|i|json!({"id":i["id"],"name":i["label"],"quantity":i["quantity"],"price":i["price"],"stock":i["stock"]})).collect::<Vec<_>>())})
}
/// The JSON fields `purchase` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PurchaseInput {
    #[serde(deserialize_with = "lenient")]
    country: Option<String>,
    #[serde(deserialize_with = "lenient")]
    budget_minor: Option<i64>,
}

pub async fn purchase<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    profile: &Value,
    key: &str,
) -> Result<Value> {
    let input: PurchaseInput = view(profile);
    let a = host.models().core;
    let cart = load_cart(a, h).await?;
    if cart.status == "completed" {
        return cart
            .data
            .order
            .clone()
            .ok_or(bad("Completed cart has no order"));
    }
    let country = input
        .country
        .as_deref()
        .ok_or(bad("Profile country required"))?;
    let (revision, minor) = review(host, h, country).await?;
    if minor > input.budget_minor.unwrap_or(0) {
        return Err(bad("Native checkout total exceeds this customer's budget"));
    }
    accept(host, h).await?;
    complete(host, h, &revision, minor, key).await
}
/// Selects simulated payment and shipping for a country; returns the reviewed revision and exact total.
pub async fn review<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    country: &str,
) -> Result<(Value, i64)> {
    let a = host.models().core;
    let cart = load_cart(a, h).await?;
    let (config, _) = scoped_config(a, &cart.tenant, &cart.data.sales_channel).await?;
    let payment = config
        .payments
        .iter()
        .find(|p| p.active && p.mode == "simulated" && !p.business_only && p.available_in(country))
        .ok_or(bad(
            "No simulated payment is configured for this customer country",
        ))?;
    let shipping = config
        .shipping
        .iter()
        .find(|s| s.active && s.countries.iter().any(|c| c == country))
        .ok_or(bad(
            "No shipping method is configured for this customer country",
        ))?;
    let region = catalogue(&config.country_definitions)
        .into_iter()
        .find(|c| c.code == country)
        .and_then(|c| c.states.first().map(|s| s.code.clone()))
        .unwrap_or_default();
    let address = json!({"firstName":"Scenario","lastName":"Shopper","street":"Teststrasse 1","zipcode":"10115","city":"Test City","country":country,"countryStateId":region});
    let quote = call(host,h,"checkout.select",&json!({"revision":cart.revision,"checkout":{"country":country,"shippingMethodId":shipping.id,"paymentMethodId":payment.id,"address":address,"billingAddress":address,"customerEmail":"scenario@example.test"}})).await?;
    let minor = quote["price"]["totalMinor"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(bad("Native quote has no exact total"))?;
    Ok((quote["revision"].clone(), minor))
}
pub async fn accept<H: AgentHost>(host: &H, h: &RequestContext) -> Result<()> {
    let policy = call(host, h, "privacy.policy", &json!({})).await?;
    call(
        host,
        h,
        "legal.accept",
        &json!({"policyVersion":policy["policyVersion"],"terms":true,"digitalImmediate":true}),
    )
    .await?;
    Ok(())
}
/// Completes with the revision and total the caller claims to have reviewed; the native owner decides.
pub async fn complete<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    revision: &Value,
    minor: i64,
    key: &str,
) -> Result<Value> {
    let mut reviewed = h.clone();
    reviewed.insert(
        "x-commerce-cart-revision",
        revision
            .to_string()
            .parse()
            .map_err(|_| bad("Invalid cart revision"))?,
    );
    reviewed.insert(
        "x-commerce-total-minor",
        minor
            .to_string()
            .parse()
            .map_err(|_| bad("Invalid exact total"))?,
    );
    call(
        host,
        &reviewed,
        "checkout.complete",
        &json!({"idempotency_key":key}),
    )
    .await
}
pub async fn products<H: AgentHost>(host: &H, h: &RequestContext, query: &str) -> Result<Value> {
    let found = call(
        host,
        h,
        "catalog.search",
        &json!({"query":query,"limit":12}),
    )
    .await?;
    let rows = found["elements"]
        .as_array()
        .or_else(|| found["products"].as_array());
    Ok(json!(rows.into_iter().flatten().map(|p|json!({"id":p["id"],"name":p["name"],"description":p["description"].as_str().unwrap_or("").chars().take(800).collect::<String>(),"price":p["price"],"stock":p["stock"]})).collect::<Vec<_>>()))
}
