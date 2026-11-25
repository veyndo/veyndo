//! Native payment-only history projection and optional owned provider observation; no financial command is executed.
use crate::domain::details_model::Details;
use crate::domain::receipt_ref::identity;
use crate::domain::{diagnostics, settlements};
use crate::ports::Gateway;
use crate::service::remote::execute;
use crate::store::attempts;
use crate::store::history::payment_history;
use crate::store::jobs;
use serde_json::{Value, json};
use veyndo_core::{Error, Result, State, StatusCode, bad, uid};

pub async fn read(gateway: &dyn Gateway, a: &State, tenant: &str, id: &str) -> Result<Value> {
    let initial = attempts::find(&a.db, tenant, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    let mut provider = json!({"status":"unsupported"});
    if initial.context["capabilities"]
        .as_array()
        .is_some_and(|cs| cs.iter().any(|c| c == "details"))
    {
        if initial.provider_order.is_none() {
            provider = json!({"status":"not_created"});
        } else {
            match execute(gateway, a, &initial, "inspect", &uid(), &json!({})).await {
                Ok(result) => {
                    identity(&initial, &result)?;
                    let details: Details = serde_json::from_value(result["details"].clone())
                        .map_err(|_| bad("Invalid provider transaction details"))?;
                    details.validate(&initial)?;
                    provider = json!({"status":"available","transaction":details});
                }
                Err(_) => {
                    provider = json!({"status":"unavailable"});
                }
            }
        }
    }
    let mut tx = a.db.begin_read_snapshot().await?;
    let p = attempts::get(&mut *tx, tenant, id).await?;
    // Financial projection only. Full order snapshots contain customer PII and keep their original history permissions.
    let history = payment_history(&mut *tx, tenant, &p.order).await?;
    let jobs = jobs::of_attempt(&mut *tx, tenant, id).await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"orderId":p.order,"provider":p.provider,"adapterVersion":p.adapter_version,"environment":p.environment,"currency":p.currency,"currencyScale":p.currency_scale,"amountMinor":p.amount,"state":p.state,"settlement":settlements::summary(&p),"providerObservation":provider,
        "history":history.iter().map(|r|json!({"id":r.id,"at":r.created_at,"state":r.state,"capturedMinor":r.captured,"refundedMinor":r.refunded})).collect::<Vec<_>>(),
        "jobs":jobs.iter().map(|r|json!({"id":r.id,"operation":r.operation,"state":r.state,"at":r.created_at,"attempts":r.attempts,"diagnosticCode":diagnostics::code(r.error.as_deref())})).collect::<Vec<_>>(),"limit":100}),
    )
}
