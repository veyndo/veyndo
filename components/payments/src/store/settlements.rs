//! Capture settlement admission: receipt claims, native job matching and the captured total in one transaction.
use crate::domain::attempt::Attempt;
use crate::domain::receipt_ref::{ProviderReceipt, is, reference};
use crate::domain::settlements::{captured, closed, multi};
use crate::store::receipt_claims::claim;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};

pub async fn admit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    context: &mut Value,
    v: &Value,
) -> Result<(i64, String, bool)> {
    let r: ProviderReceipt = view(v);
    if !r.confirmed() {
        return Err(bad("Settlement must be provider confirmed"));
    }
    let legacy;
    let records = if let Some(records) = v.get("captures") {
        records
            .as_array()
            .filter(|rs| !rs.is_empty() && rs.len() <= 1000)
            .ok_or(bad("Invalid provider capture collection"))?
    } else {
        if r.settled_amount_minor != Some(p.amount) {
            return Err(bad("Legacy settlement must match the full invoice"));
        }
        legacy = vec![json!({"id":r.capture_id,"amountMinor":p.amount,"currency":p.currency})];
        &legacy
    };
    let mut receipts = context["captureReceipts"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if receipts.is_empty()
        && let Some(id) = &p.capture
    {
        receipts.insert(
            id.clone(),
            json!({"amountMinor":captured(p),"currency":p.currency}),
        );
    }
    let mut seen = std::collections::HashSet::new();
    let mut snapshot = 0_i64;
    for record in records {
        let id = reference(&record["id"])?;
        let amount = record["amountMinor"]
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or(bad("Positive capture receipt amount required"))?;
        if !seen.insert(id) || record["currency"] != p.currency {
            return Err(bad("Duplicate capture receipt or currency mismatch"));
        }
        snapshot = snapshot
            .checked_add(amount)
            .filter(|n| *n <= p.amount)
            .ok_or(bad("Provider captures exceed invoice"))?;
        if let Some(old) = receipts.get(id)
            && (old["amountMinor"] != amount || old["currency"] != p.currency)
        {
            return Err(conflict("Capture receipt amount changed"));
        }
        claim(tx, p, "capture", id, "").await?;
        if let Some(key) = record.get("requestKey") {
            let key = reference(key)?;
            let row = sqlx::query("SELECT operation,request,state FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND id=$3 FOR UPDATE")
                .bind(&p.tenant).bind(&p.id).bind(key).fetch_optional(&mut **tx).await?
                .ok_or(bad("Capture has no matching native request"))?;
            let request: Value = row.get("request");
            if row.get::<String, _>("operation") != "capture"
                || request
                    .get("amountMinor")
                    .map(|n| n.as_i64())
                    .unwrap_or(Some(p.amount))
                    != Some(amount)
            {
                return Err(conflict("Capture changed its native requested amount"));
            }
            claim(tx, p, "capture-job", key, id).await?;
            sqlx::query("UPDATE payment_jobs SET state='succeeded',response=$1,error=NULL,lease_until=NULL WHERE tenant=$2 AND attempt_id=$3 AND id=$4 AND state IN ('queued','running','uncertain')")
                .bind(v).bind(&p.tenant).bind(&p.id).bind(key).execute(&mut **tx).await?;
        }
        receipts.insert(
            id.into(),
            json!({"amountMinor":amount,"currency":p.currency}),
        );
    }
    if receipts.len() > 1000 {
        return Err(bad("Too many capture receipts for one payment"));
    }
    if r.settled_amount_minor != Some(snapshot) {
        return Err(bad("Capture collection does not match settlement total"));
    }
    let mut total = 0_i64;
    for record in receipts.values() {
        total = total
            .checked_add(record["amountMinor"].as_i64().unwrap_or(-1))
            .filter(|n| *n > 0 && *n <= p.amount)
            .ok_or(bad("Captures exceed original invoice"))?;
    }
    if (snapshot < p.amount || receipts.len() > 1) && !multi(p) {
        return Err(bad(
            "Partial or multiple capture is not enabled for this attempt",
        ));
    }
    if snapshot < p.amount || receipts.len() > 1 {
        let authorization = context["authorizationId"]
            .as_str()
            .or(r.authorization_id.as_str())
            .ok_or(bad(
                "Partial capture requires confirmed authorization evidence",
            ))?
            .to_owned();
        reference(&json!(authorization))?;
        if r.authorization_id
            .as_str()
            .is_some_and(|id| id != authorization)
            || (context["authorizationId"].is_null() || v.get("authorizedAmountMinor").is_some())
                && r.authorized_amount_minor != Some(p.amount)
        {
            return Err(bad("Partial capture authorization changed"));
        }
        claim(tx, p, "authorization", &authorization, "").await?;
        context["authorizationId"] = json!(authorization);
    }
    if snapshot < p.amount && !is(&r.status, "partially_captured") {
        return Err(bad("Partial settlement cannot report a full capture"));
    }
    let closed = closed(p) || total == p.amount || r.capture_closed == Some(true);
    let first = p
        .capture
        .clone()
        .unwrap_or_else(|| records[0]["id"].as_str().unwrap().to_owned());
    context["captureReceipts"] = json!(receipts);
    context["capturedAmountMinor"] = json!(total);
    context["captureClosed"] = json!(closed);
    Ok((total, first, closed))
}
