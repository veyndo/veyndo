//! Typed recipient instructions of an attempt: the stored snapshot or the provider's current answer; reads never settle an invoice.
use crate::domain::instructions::{Instructions, snapshot, validated};
use crate::domain::receipt_ref::identity;
use crate::ports::Gateway;
use crate::service::remote::execute;
use crate::store::attempts;
use crate::store::commands::customer;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, uid};
use veyndo_identity::{merchant, permit};

pub async fn read(gateway: &dyn Gateway, a: &State, t: &str, id: &str) -> Result<Value> {
    let p = attempts::find(&a.db, t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    if !p.context["capabilities"]
        .as_array()
        .is_some_and(|cs| cs.iter().any(|c| c == "instructions"))
    {
        return Ok(json!({"status":"unsupported","instructions":null}));
    }
    if let Some(saved) = p.context.get("recipientInstructions") {
        return Ok(
            json!({"status":"available","instructions":validated(saved)?,"amountMinor":p.amount,"currency":p.currency,"currencyScale":p.currency_scale,"simulated":p.environment!="live"}),
        );
    }
    let v = execute(gateway, a, &p, "instructions", &uid(), &json!({})).await?;
    identity(&p, &v)?;
    let instructions = match v["status"].as_str() {
        Some("pending") if v["instructions"].is_null() => None,
        Some("available") => {
            let i: Instructions = serde_json::from_value(v["instructions"].clone())
                .map_err(|_| bad("Invalid recipient instructions response"))?;
            i.validate()?;
            Some(i)
        }
        _ => return Err(bad("Invalid recipient instructions state")),
    };
    Ok(
        json!({"status":v["status"],"instructions":instructions,"amountMinor":p.amount,"currency":p.currency,"currencyScale":p.currency_scale,"simulated":p.environment!="live"}),
    )
}

/// A customer reads the instructions of a payment bound to their own cart.
pub async fn customer_read(
    gateway: &dyn Gateway,
    a: &State,
    h: &RequestContext,
    id: &str,
) -> Result<Value> {
    customer(a, h, id).await?;
    read(gateway, a, &tenant(h)?, id).await
}

/// Merchant reads need the compound payment, order and PII rights.
pub async fn merchant_read(
    gateway: &dyn Gateway,
    a: &State,
    h: &RequestContext,
    id: &str,
) -> Result<Value> {
    let t = compound(h)?;
    read(gateway, a, &t, id).await
}

fn compound(h: &RequestContext) -> Result<String> {
    let t = merchant(h)?;
    for scope in ["payments.read", "orders.read", "customers.pii"] {
        permit(h, scope)?;
    }
    Ok(t)
}

/// The issuer already holds the order lock and document/PII authority. This immutable read takes no reverse payment lock and performs no provider I/O.
/// The order and payment attempt a document carries instructions for.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DocumentOrder {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    payment: DocumentPayment,
}

/// The payment attempt of an order.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DocumentPayment {
    #[serde(deserialize_with = "lenient")]
    attempt_id: Option<String>,
}

pub async fn for_document(
    tx: &mut sqlx::PgConnection,
    t: &str,
    order: &Value,
) -> Result<Option<Value>> {
    let order: DocumentOrder = view(order);
    let Some(id) = order.payment.attempt_id.as_deref() else {
        return Ok(None);
    };
    let p = attempts::find_for_order(tx, t, id, order.id.as_deref().unwrap_or(""))
        .await?
        .ok_or(bad("Document payment is not bound to this order"))?;
    if !p.context["capabilities"]
        .as_array()
        .is_some_and(|caps| caps.iter().any(|cap| cap == "instructions"))
    {
        return Ok(None);
    }
    let value=p.context.get("recipientInstructions").ok_or(conflict("Invoice payment instructions are still pending; check the payment before issuing the invoice"))?;
    snapshot(&p, value).map(Some)
}

/// Flow actions recheck current compound rights and fetch the original order's cached evidence at execution time.
pub async fn for_flow(a: &State, h: &RequestContext, order: &str) -> Result<Value> {
    let t = compound(h)?;
    let p = attempts::current_of_order(&a.db, &t, order).await?;
    snapshot(
        &p,
        p.context
            .get("recipientInstructions")
            .ok_or(bad("Payment instructions unavailable"))?,
    )
}
