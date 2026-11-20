//! Typed recipient instructions of an attempt: the stored snapshot or the provider's current answer; reads never settle an invoice.
use crate::domain::instructions::{Instructions, validated};
use crate::domain::receipt_ref::identity;
use crate::ports::Gateway;
use crate::service::remote::execute;
use crate::store::attempts::attempt;
use serde_json::{Value, json};
use veyndo_core::{Error, Result, State, StatusCode, bad, uid};

pub async fn read(gateway: &dyn Gateway, a: &State, t: &str, id: &str) -> Result<Value> {
    let row = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    let p = attempt(&row);
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
