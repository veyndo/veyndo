//! Customer-capability and compound merchant access to typed recipient instructions; reads never settle an invoice.
use super::{attempt, customer, generic_receipts, remote};
use crate::{
    App, Json, Path, RequestContext, Result, State, Value, auth, bad, json, merchant, tenant, uid,
};
use veyndo_payments::domain::instructions::{Instructions, validated};
pub(crate) async fn customer_read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    customer(a, h, id).await?;
    read(a, &tenant(h)?, id).await
}
pub(crate) async fn merchant_read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    let t = merchant(a, h)?;
    for scope in ["payments.read", "orders.read", "customers.pii"] {
        auth::permit(h, scope)?;
    }
    read(a, &t, id).await
}
pub(crate) async fn customer_route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(customer_read(&a, &h, &id).await?))
}
pub(crate) async fn merchant_route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(merchant_read(&a, &h, &id).await?))
}
async fn read(a: &App, t: &str, id: &str) -> Result<Value> {
    let row = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(crate::Error(
            crate::StatusCode::NOT_FOUND,
            "Payment not found".into(),
        ))?;
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
    let v = remote::execute(a, &p, "instructions", &uid(), &json!({})).await?;
    generic_receipts::identity(&p, &v)?;
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
