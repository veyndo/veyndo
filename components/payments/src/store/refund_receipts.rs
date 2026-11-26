//! Exactly-once provider refund admission, including refunds initiated outside Studio.
use crate::domain::attempt::{Attempt, context_of};
use crate::domain::receipt_ref::{RefundReceipt, is, reference};
use crate::domain::refunds::{self, checked_total};
use crate::domain::settlements;
use crate::store::receipt_claims::claim;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::document_slot::field;
use veyndo_core::{Result, bad, conflict};

/// One provider refund receipt of an attempt, keyed by its provider id.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRefund<'a> {
    amount_minor: i64,
    job_key: &'a str,
    capture_id: &'a str,
}

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
        let expected: Option<Value> = sqlx::query_scalar!("SELECT request FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND id=$3 AND operation='refund'", p.tenant, p.id, key).fetch_optional(&mut **tx).await?;
        if expected
            .map(|v| view::<RefundReceipt>(&v))
            .is_none_or(|job| {
                job.amount_minor != Some(amount) || job.capture_id.is_some_and(|c| c != capture)
            })
        {
            return Err(bad("Provider refund does not match its native job"));
        }
    }
    let account = p.ctx().account_ref;
    let old = sqlx::query!("SELECT tenant,attempt_id,job_key FROM payment_receipt_claims WHERE provider=$1 AND environment=$2 AND account_ref=$3 AND kind='refund' AND receipt_ref=$4 FOR UPDATE", p.provider, p.environment, account.as_str().unwrap_or(""), id).fetch_optional(&mut **tx).await?;
    if let Some(old) = old {
        let saved_key: String = old.job_key;
        if old.tenant != p.tenant
            || old.attempt_id != p.id
            || key.is_some_and(|key| key != saved_key)
        {
            return Err(conflict("Provider refund receipt already allocated"));
        }
        let recorded: RecordedRefund = context_of(context)
            .provider_refunds
            .get(id)
            .map(view)
            .unwrap_or_default();
        let saved = recorded.amount_minor.as_i64();
        let historical: Option<i64> = if saved.is_none() {
            sqlx::query_scalar!(r#"SELECT (request->>'amountMinor')::bigint AS "amount!" FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND id=$3 AND operation='refund'"#, p.tenant, p.id, saved_key).fetch_optional(&mut **tx).await?
        } else {
            None
        };
        if saved.or(historical) != Some(amount)
            || recorded.capture_id.as_str().is_some_and(|c| c != capture)
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
    if !context_of(context).provider_refunds.is_object() {
        *field(context, "providerRefunds")? = Value::Object(serde_json::Map::new());
    }
    if context_of(context)
        .provider_refunds
        .as_object()
        .is_some_and(|rows| rows.len() >= 1000)
    {
        return Err(bad("Too many refund receipts for one payment"));
    }
    *field(field(context, "providerRefunds")?, id)? = json!(ProviderRefund {
        amount_minor: amount,
        job_key: key,
        capture_id: &capture,
    });
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
    sqlx::query!("UPDATE payment_jobs SET state='succeeded',response=$1,error=NULL,lease_until=NULL WHERE tenant=$2 AND attempt_id=$3 AND id=$4 AND operation='refund' AND state IN ('queued','running','uncertain')", json!({"confirmed":true,"refund":{"id":id,"amountMinor":amount,"currency":p.currency,"captureId":capture}}), p.tenant, p.id, key).execute(&mut **tx).await?;
    Ok(())
}

/// A provider refund as a receipt lists it or the context records it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecordedRefund {
    id: Value,
    amount_minor: Value,
    capture_id: Value,
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
        let listed: RecordedRefund = view(refund);
        let id = reference(&listed.id)?;
        if !seen.insert(id.to_owned()) {
            return Err(bad("Duplicate provider refund receipt"));
        }
        let captured = context_of(context)
            .captured_amount_minor
            .as_i64()
            .unwrap_or_else(|| settlements::captured(p));
        total = checked_total(captured, total, apply(tx, p, context, refund, None).await?)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::ProviderRefund;
    use serde_json::{Value, json};
    use veyndo_core::document_slot::field;

    /// The typed refund receipt writes exactly the provider context the indexed assignments wrote.
    #[test]
    fn the_typed_refund_receipt_matches_the_indexed_write() {
        for base in [
            json!({"method":"card"}),
            json!({"providerRefunds":null}),
            json!({"providerRefunds":{"r0":{}}}),
        ] {
            let mut indexed = base.clone();
            if !indexed["providerRefunds"].is_object() {
                indexed["providerRefunds"] = json!({});
            }
            indexed["providerRefunds"]["r1"] =
                json!({"amountMinor":250,"jobKey":"job","captureId":"c1"});
            let mut typed = base.clone();
            if !typed.get("providerRefunds").is_some_and(Value::is_object) {
                *field(&mut typed, "providerRefunds").unwrap() =
                    Value::Object(serde_json::Map::new());
            }
            *field(field(&mut typed, "providerRefunds").unwrap(), "r1").unwrap() =
                json!(ProviderRefund {
                    amount_minor: 250,
                    job_key: "job",
                    capture_id: "c1"
                });
            assert_eq!(typed, indexed);
        }
    }
}
