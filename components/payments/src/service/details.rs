//! Native payment-only history projection and optional owned provider observation; no financial command is executed.
use crate::domain::details_model::Details;
use crate::domain::receipt_ref::identity;
use crate::domain::{diagnostics, settlements};
use crate::ports::Gateway;
use crate::service::remote::execute;
use crate::store::attempts::attempt;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, Result, State, StatusCode, bad, uid};

pub async fn read(gateway: &dyn Gateway, a: &State, tenant: &str, id: &str) -> Result<Value> {
    let row = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    let initial = attempt(&row);
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
    let row = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let p = attempt(&row);
    // Financial projection only. Full order snapshots contain customer PII and keep their original history permissions.
    let history=sqlx::query("SELECT id,to_jsonb(created_at)#>>'{}' AS created_at,after_state#>>'{payment,state}' AS state,after_state#>'{payment,capturedMinor}' AS captured,after_state#>'{payment,refundedMinor}' AS refunded FROM entity_history WHERE tenant=$1 AND entity='order' AND entity_id=$2 AND finalized AND (before_state->'payment') IS DISTINCT FROM (after_state->'payment') ORDER BY id DESC LIMIT 100")
        .bind(tenant).bind(&p.order).fetch_all(&mut *tx).await?;
    let jobs=sqlx::query("SELECT id,operation,state,to_jsonb(created_at)#>>'{}' AS created_at,attempts,error FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 ORDER BY created_at DESC,id DESC LIMIT 100")
        .bind(tenant).bind(id).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"orderId":p.order,"provider":p.provider,"adapterVersion":p.adapter_version,"environment":p.environment,"currency":p.currency,"currencyScale":p.currency_scale,"amountMinor":p.amount,"state":p.state,"settlement":settlements::summary(&p),"providerObservation":provider,
        "history":history.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"at":r.get::<String,_>("created_at"),"state":r.get::<Option<String>,_>("state"),"capturedMinor":r.get::<Option<Value>,_>("captured"),"refundedMinor":r.get::<Option<Value>,_>("refunded")})).collect::<Vec<_>>(),
        "jobs":jobs.iter().map(|r|json!({"id":r.get::<String,_>("id"),"operation":r.get::<String,_>("operation"),"state":r.get::<String,_>("state"),"at":r.get::<String,_>("created_at"),"attempts":r.get::<i32,_>("attempts"),"diagnosticCode":diagnostics::code(r.get::<Option<String>,_>("error").as_deref())})).collect::<Vec<_>>(),"limit":100}),
    )
}
