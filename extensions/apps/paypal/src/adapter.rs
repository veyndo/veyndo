//! Legacy PayPal admission/account wiring belongs to the PayPal package, not a second pricing owner.
use crate::compatibility as paypal;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::State;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::uid;
use veyndo_payments::domain::attempt::Attempt;
use veyndo_payments::store::jobs::enqueue_tx;
use veyndo_payments::store::provider_registry::prepare_remote;
use veyndo_shop::types::cart::StoredCart;
/// The configured PayPal account of a shop.
#[derive(Clone)]
pub struct Account {
    pub client: String,
    pub secret: String,
    pub webhook: String,
    pub merchant: Option<String>,
    pub bn_code: String,
}
/// The PayPal account the operator configured for the shop.
pub fn account(t: &str) -> Result<Account> {
    let all = &veyndo_core::process_config::get().paypal_accounts;
    let v = &all[t];
    Ok(Account {
        client: v["clientId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Connect this shop's PayPal Sandbox account on the server first".into(),
            ))?
            .into(),
        secret: v["clientSecret"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(bad("Payment secret missing"))?
            .into(),
        webhook: v["webhookId"].as_str().unwrap_or("").into(),
        merchant: v["merchantId"].as_str().map(str::to_string),
        bn_code: v["bnCode"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(bad("Provider attribution configuration missing"))?
            .to_string(),
    })
}
/// The PayPal API base URL of the configured environment.
pub fn base() -> Result<String> {
    Ok(veyndo_core::process_config::get().paypal_base.clone())
}
/// The configured PayPal environment (`sandbox` or `live`).
pub fn environment() -> &'static str {
    veyndo_core::process_config::get().paypal_environment
}
/// Creates the pending PayPal attempt of a new order, its inventory reservations and the create job; adapter providers go through the generic path.
pub async fn prepare(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &StoredCart,
    order: &mut Value,
    minor: i64,
    return_origin: Option<&str>,
) -> Result<()> {
    if order["cart"]["paymentMethod"]["provider"].is_string() {
        return prepare_remote(tx, c, order, minor, return_origin).await;
    }
    let version: Option<String> = sqlx::query_scalar(
        "SELECT version FROM app_packages WHERE tenant=$1 AND id='paypal' AND active FOR SHARE",
    )
    .bind(&c.tenant)
    .fetch_optional(&mut **tx)
    .await?;
    let version = version.ok_or(conflict("Payment app is not active"))?;
    let id = uid();
    let bn = account(&c.tenant)?.bn_code;
    order["payment"] = json!({"method":order["cart"]["paymentMethod"],"provider":"paypal","state":"pending","attemptId":id,"environment":environment(),"realMoneyCharged":false,"bnCode":bn});
    sqlx::query("INSERT INTO payment_attempts(id,tenant,order_id,provider,adapter_version,amount_minor,currency,environment,bn_code,provider_context,currency_scale) VALUES($1,$2,$3,'paypal',$4,$5,$8,$6,$7,$9,$10)").bind(&id).bind(&c.tenant).bind(order["id"].as_str().unwrap()).bind(version).bind(minor).bind(environment()).bind(&bn).bind(order["cart"]["price"]["currency"].as_str().unwrap_or("EUR")).bind(json!({"currencyScale":order["cart"]["price"]["currencyScale"]})).bind(order["cart"]["price"]["currencyScale"].as_i64().unwrap_or(2) as i16).execute(&mut **tx).await?;
    for item in &c.data.items {
        sqlx::query("INSERT INTO inventory_reservations(tenant,attempt_id,product_id,quantity) VALUES($1,$2,$3,$4)").bind(&c.tenant).bind(&id).bind(&item.id).bind(item.quantity as i32).execute(&mut **tx).await?;
    }
    enqueue_tx(
        tx,
        &c.tenant,
        &id,
        "create",
        &format!("{id}:create"),
        &json!({}),
    )
    .await?;
    Ok(())
}
/// Executes a payment operation at PayPal; an attempt made under another adapter version, environment or attribution code is refused.
pub async fn execute(a: &State, p: &Attempt, op: &str, key: &str, input: &Value) -> Result<Value> {
    if p.adapter_version != "1.0.0"
        || p.environment != environment()
        || p.bn_code != account(&p.tenant)?.bn_code
    {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Payment adapter version/environment mismatch; use the original adapter to reconcile"
                .into(),
        ));
    }
    paypal::execute(a, p, op, key, input).await
}
