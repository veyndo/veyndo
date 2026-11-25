//! A closed shopper adapter uses native carts and exact reviewed totals; no model-supplied authority or payment mode.
use crate::ports::AgentHost;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, lenient_list, view};
use veyndo_shipping::domain::geography::catalogue;
use veyndo_shop::service::read_cache::settings as scoped_config;
/// The language, channel and currency a fictional shopper uses.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ShopperRun {
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel: Option<String>,
    #[serde(deserialize_with = "lenient")]
    scenario: ScenarioCurrency,
}

/// The currency of a scenario.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ScenarioCurrency {
    #[serde(deserialize_with = "lenient")]
    currency: Option<String>,
}

pub fn headers(t: &str, request: &Value, token: Option<&str>) -> Result<RequestContext> {
    let run: ShopperRun = view(request);
    let mut h = RequestContext::new();
    for (key, value) in [
        ("x-tenant", t),
        (
            "x-commerce-locale",
            run.locale.as_deref().unwrap_or("en-GB"),
        ),
        (
            "sw-sales-channel-id",
            run.channel.as_deref().unwrap_or("default"),
        ),
        (
            "x-commerce-currency",
            run.scenario.currency.as_deref().unwrap_or(""),
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
        "items":args.line_items.as_array().map(|items|items.iter().map(|i|{let i=view::<ObservedLine>(i);json!({"id":i.id,"name":i.label,"quantity":i.quantity,"price":i.price,"stock":i.stock})}).collect::<Vec<_>>())})
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
    let quote: Reviewed = view(&quote);
    let minor = quote
        .price
        .total_minor
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(bad("Native quote has no exact total"))?;
    Ok((quote.revision, minor))
}
pub async fn accept<H: AgentHost>(host: &H, h: &RequestContext) -> Result<()> {
    let policy = call(host, h, "privacy.policy", &json!({})).await?;
    call(
        host,
        h,
        "legal.accept",
        &json!({"policyVersion":view::<Policy>(&policy).policy_version,"terms":true,"digitalImmediate":true}),
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
    let found: Found = view(&found);
    let rows = found.elements.or(found.products);
    Ok(json!(rows.into_iter().flatten().map(|p|{let p=view::<Listed>(&p);json!({"id":p.id,"name":p.name,"description":p.description.as_deref().unwrap_or("").chars().take(800).collect::<String>(),"price":p.price,"stock":p.stock})}).collect::<Vec<_>>()))
}

/// A quoted cart line as a simulated customer observes it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ObservedLine {
    id: Value,
    label: Value,
    quantity: Value,
    price: Value,
    stock: Value,
}

/// A reviewed checkout quote.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Reviewed {
    revision: Value,
    #[serde(deserialize_with = "lenient")]
    price: ReviewedPrice,
}

/// The exact total of a reviewed quote.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewedPrice {
    #[serde(deserialize_with = "lenient")]
    total_minor: Option<String>,
}

/// The privacy policy version a customer accepts.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Policy {
    policy_version: Value,
}

/// A catalogue search result in either list shape.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Found {
    #[serde(deserialize_with = "lenient")]
    elements: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    products: Option<Vec<Value>>,
}

/// A listed product as a simulated customer sees it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Listed {
    id: Value,
    name: Value,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
    price: Value,
    stock: Value,
}

/// A product of the observed catalogue, as probes pick one.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Stocked {
    #[serde(deserialize_with = "lenient")]
    pub id: Option<String>,
    pub name: Value,
    #[serde(deserialize_with = "lenient")]
    pub stock: Option<i64>,
}

/// A cart line's quantity after a change.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ChangedLine {
    #[serde(deserialize_with = "lenient")]
    pub quantity: Option<i64>,
}

/// A changed cart's lines.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Changed {
    #[serde(deserialize_with = "lenient_list")]
    pub line_items: Vec<ChangedLine>,
}

/// A created cart's capability, or a placed order's ID.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Created {
    #[serde(deserialize_with = "lenient")]
    pub token: Option<String>,
    pub id: Value,
}

/// The outcome of one probe.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Outcome {
    pub outcome: Value,
}
