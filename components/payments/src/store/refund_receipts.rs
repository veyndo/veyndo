//! Exactly-once provider refund admission, including refunds initiated outside Studio.
use crate::domain::attempt::Attempt;
use crate::domain::receipt_ref::{RefundReceipt, is, reference};
use crate::domain::refunds::{self, checked_total};
use crate::domain::settlements;
use crate::store::receipt_claims::claim;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};

pub async fn apply(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    context: &mut Value,
    refund: &Value,
    native_key: Option<&str>,
) -> Result<i64> {
    let receipt: RefundReceipt = view(refund);
    let id = reference(&receipt.id)?;
    let (capture, capture_amount) = refunds::capture(p, context, refund)?;
    let amount = receipt
        .amount_minor
        .filter(|n| *n > 0 && *n <= capture_amount)
        .ok_or(bad("Invalid provider refund amount"))?;
    if !is(&receipt.currency, &p.currency) {
        return Err(bad("Provider refund currency mismatch"));
    }
    let key = native_key.or(receipt.request_key.as_deref());
    if let Some(key) = key {
        let expected: Option<Value> = sqlx::query_scalar("SELECT request FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND id=$3 AND operation='refund'")
            .bind(&p.tenant).bind(&p.id).bind(key).fetch_optional(&mut **tx).await?;
        if expected
            .map(|v| view::<RefundReceipt>(&v))
            .is_none_or(|job| {
                job.amount_minor != Some(amount) || job.capture_id.is_some_and(|c| c != capture)
            })
        {
            return Err(bad("Provider refund does not match its native job"));
        }
    }
    let old = sqlx::query("SELECT tenant,attempt_id,job_key FROM payment_receipt_claims WHERE provider=$1 AND environment=$2 AND account_ref=$3 AND kind='refund' AND receipt_ref=$4 FOR UPDATE")
        .bind(&p.provider).bind(&p.environment).bind(p.context["accountRef"].as_str().unwrap_or(""))
        .bind(id).fetch_optional(&mut **tx).await?;
    if let Some(old) = old {
        let saved_key: String = old.get("job_key");
        if old.get::<String, _>("tenant") != p.tenant
            || old.get::<String, _>("attempt_id") != p.id
            || key.is_some_and(|key| key != saved_key)
        {
            return Err(conflict("Provider refund receipt already allocated"));
        }
        let saved = context["providerRefunds"][id]["amountMinor"].as_i64();
        let historical: Option<i64> = if saved.is_none() {
            sqlx::query_scalar("SELECT (request->>'amountMinor')::bigint FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND id=$3 AND operation='refund'")
                .bind(&p.tenant).bind(&p.id).bind(&saved_key).fetch_optional(&mut **tx).await?
        } else {
            None
        };
        if saved.or(historical) != Some(amount)
            || context["providerRefunds"][id]["captureId"]
                .as_str()
                .is_some_and(|c| c != capture)
        {
            return Err(conflict("Provider refund receipt amount changed"));
        }
        if let Some(key) = key {
            finish_native(tx, p, key, id, amount, &capture).await?;
        }
        return Ok(0);
    }
    let native = key;
    let external_key = format!("provider-refund:{id}");
    let key = key.unwrap_or(&external_key);
    checked_total(capture_amount, refunds::used(p, context, &capture), amount)?;
    claim(tx, p, "refund", id, key).await?;
    if !context["providerRefunds"].is_object() {
        context["providerRefunds"] = json!({});
    }
    if context["providerRefunds"]
        .as_object()
        .is_some_and(|rows| rows.len() >= 1000)
    {
        return Err(bad("Too many refund receipts for one payment"));
    }
    context["providerRefunds"][id] = json!({"amountMinor":amount,"jobKey":key,"captureId":capture});
    if let Some(key) = native {
        finish_native(tx, p, key, id, amount, &capture).await?;
    }
    Ok(amount)
}

async fn finish_native(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    key: &str,
    id: &str,
    amount: i64,
    capture: &str,
) -> Result<()> {
    sqlx::query("UPDATE payment_jobs SET state='succeeded',response=$1,error=NULL,lease_until=NULL WHERE tenant=$2 AND attempt_id=$3 AND id=$4 AND operation='refund' AND state IN ('queued','running','uncertain')")
        .bind(json!({"confirmed":true,"refund":{"id":id,"amountMinor":amount,"currency":p.currency,"captureId":capture}}))
        .bind(&p.tenant).bind(&p.id).bind(key).execute(&mut **tx).await?;
    Ok(())
}

pub async fn reconcile(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    context: &mut Value,
    response: &Value,
) -> Result<i64> {
    let Some(refunds) = response.get("refunds") else {
        return Ok(p.refunded);
    };
    let refunds = refunds
        .as_array()
        .filter(|rs| rs.len() <= 1000)
        .ok_or(bad("Invalid provider refund collection"))?;
    let mut seen = std::collections::HashSet::new();
    let mut total = p.refunded;
    for refund in refunds {
        let id = reference(&refund["id"])?;
        if !seen.insert(id) {
            return Err(bad("Duplicate provider refund receipt"));
        }
        let captured = context["capturedAmountMinor"]
            .as_i64()
            .unwrap_or_else(|| settlements::captured(p));
        total = checked_total(captured, total, apply(tx, p, context, refund, None).await?)?;
    }
    Ok(total)
}
