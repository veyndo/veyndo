//! Idempotent immutable invoices/delivery notes with transactional per-shop number ranges.
use crate::ports::Payments;
use crate::store::company;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, uid};

/// The receipts issued for an order, newest first.
pub async fn list(a: &State, t: &str, id: &str) -> Result<Value> {
    let rows=sqlx::query("SELECT id,kind,number,locale,created_at::text AS created FROM order_receipts WHERE tenant=$1 AND order_id=$2 ORDER BY created_at DESC LIMIT 100").bind(t).bind(id).fetch_all(&a.db).await?;
    Ok(
        json!({"elements":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"kind":r.get::<String,_>("kind"),"number":r.get::<String,_>("number"),"locale":r.get::<String,_>("locale"),"createdAt":r.get::<String,_>("created"),"pdfPath":format!("/api/merchant/receipts/{}/pdf",r.get::<String,_>("id"))})).collect::<Vec<_>>() }),
    )
}
/// Issues one immutable receipt for the order, or returns the one the request key already issued.
pub async fn create(
    payments: &dyn Payments,
    a: &State,
    h: &RequestContext,
    t: &str,
    id: &str,
    v: &Value,
) -> Result<Value> {
    let key = v["requestKey"]
        .as_str()
        .or_else(|| header(h, "idempotency-key"))
        .filter(|s| (8..=128).contains(&s.len()))
        .ok_or(bad("requestKey must contain 8..128 characters"))?;
    let kind = v["kind"]
        .as_str()
        .filter(|s| ["invoice", "delivery_note", "cancellation"].contains(s))
        .ok_or(bad("Invalid receipt kind"))?;
    let locale = v["locale"].as_str().unwrap_or("en");
    if !["en", "de", "fr", "es"].contains(&locale) {
        return Err(bad("Unsupported receipt locale"));
    }
    // Preserve existing whole-order retry identities across deployment. Package
    // selection is additive and must never alias a historical whole-order note.
    let reference = if v.get("deliveryIndex").is_some() {
        json!({"reference":v["referenceId"],"deliveryIndex":v["deliveryIndex"]})
    } else {
        v["referenceId"].clone()
    };
    let fingerprint = hash(&format!(
        "{id}:{kind}:{locale}:{}:{}",
        v["revision"], reference
    ));
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("receipt:{t}:{key}"))
        .execute(&mut *tx)
        .await?;
    if let Some(r) = sqlx::query(
        "SELECT id,number,fingerprint FROM order_receipts WHERE tenant=$1 AND request_key=$2",
    )
    .bind(t)
    .bind(key)
    .fetch_optional(&mut *tx)
    .await?
    {
        if r.get::<String, _>("fingerprint") != fingerprint {
            return Err(conflict("Receipt request key reused"));
        }
        return Ok(json!({"id":r.get::<String,_>("id"),"number":r.get::<String,_>("number")}));
    }
    let r=sqlx::query("SELECT data #- '{cart,token}' AS data,created_at::text AS created FROM orders WHERE tenant=$1 AND id=$2 FOR UPDATE").bind(t).bind(id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Order not found".into()))?;
    let order: Value = r.get("data");
    if !order["historicalImport"].is_null() {
        return Err(conflict(
            "Historical invoices must be imported as original documents; no new invoice may be issued",
        ));
    }
    if order["revision"] != v["revision"] {
        return Err(conflict("Order revision changed"));
    }
    let package = if let Some(index) = v.get("deliveryIndex") {
        if kind != "delivery_note" {
            return Err(bad("Only delivery notes select a package"));
        }
        let index = index
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(bad("Invalid delivery note package"))?;
        Some(
            order["deliveries"]
                .as_array()
                .and_then(|v| v.get(index))
                .ok_or(bad("Delivery note package unavailable"))?
                .clone(),
        )
    } else {
        None
    };
    let seller = company::seller(
        &mut tx,
        t,
        order["salesChannelId"].as_str().unwrap_or("default"),
    )
    .await?;
    let reference = if kind == "cancellation" {
        let ref_id = v["referenceId"]
            .as_str()
            .ok_or(bad("Original invoice reference required"))?;
        let original:Value=sqlx::query_scalar("SELECT snapshot FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND id=$3 AND kind='invoice'").bind(t).bind(id).bind(ref_id).fetch_optional(&mut *tx).await?.ok_or(bad("Original invoice unavailable"))?;
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM order_receipts WHERE tenant=$1 AND snapshot->>'referenceId'=$2 AND kind='cancellation')").bind(t).bind(ref_id).fetch_one(&mut *tx).await?;
        if exists {
            return Err(conflict("Invoice already cancelled"));
        }
        Some(original)
    } else {
        None
    };
    // A single original invoice per order; cancellation does not initiate a provider refund.
    if kind == "invoice" {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND kind='invoice')").bind(t).bind(id).fetch_one(&mut *tx).await?;
        if exists {
            return Err(conflict("Invoice already exists"));
        }
    }
    let payment_instructions = if kind == "invoice" {
        payments.document_instructions(&mut tx, t, &order).await?
    } else {
        None
    };
    let number:i64=sqlx::query_scalar("INSERT INTO receipt_counters(tenant,kind,next_number) VALUES($1,$2,2) ON CONFLICT(tenant,kind) DO UPDATE SET next_number=receipt_counters.next_number+1 RETURNING next_number-1").bind(t).bind(kind).fetch_one(&mut *tx).await?;
    let prefix = match kind {
        "invoice" => "INV",
        "delivery_note" => "DEL",
        _ => "CAN",
    };
    let number = format!("{prefix}-{number:08}");
    let receipt = uid();
    let mut snapshot = json!({"number":number,"kind":kind,"locale":locale,"seller":if let Some(ref original)=reference{original["seller"].clone()}else{seller},"order":if let Some(ref original)=reference{original["order"].clone()}else{order.clone()},"issuedAt":sqlx::query_scalar::<_,String>("SELECT now()::text").fetch_one(&mut *tx).await?,"referenceId":v["referenceId"],"referenceNumber":reference.map(|s|s["number"].clone())});
    if let Some(instructions) = payment_instructions {
        snapshot["paymentInstructions"] = instructions;
    }
    if let Some(package) = package {
        snapshot["deliveryIndex"] = v["deliveryIndex"].clone();
        snapshot["delivery"] = package;
    }
    sqlx::query("INSERT INTO order_receipts(id,tenant,order_id,kind,number,locale,snapshot,request_key,fingerprint) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(&receipt).bind(t).bind(id).bind(kind).bind(&number).bind(locale).bind(snapshot).bind(key).bind(fingerprint).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'receipt',$4)",
    )
    .bind(t)
    .bind(id)
    .bind(header(h, "x-rac-user").unwrap())
    .bind(json!({"id":receipt,"number":number,"kind":kind}))
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'document.created',$2)")
        .bind(t)
        .bind(json!({"orderId":id,"documentId":receipt,"kind":kind}))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(
        json!({"id":receipt,"number":number,"pdfPath":format!("/api/merchant/receipts/{receipt}/pdf")}),
    )
}

/// The frozen snapshot of a receipt and, for an imported original, its stored document.
pub async fn stored(a: &State, t: &str, id: &str) -> Result<(Value, Option<Vec<u8>>)> {
    let r = sqlx::query("SELECT snapshot,content FROM order_receipts WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Receipt not found".into()))?;
    Ok((r.get("snapshot"), r.get("content")))
}

/// Scoped immutable document export for separately consented adapters; never creates a new fiscal document.
pub async fn snapshot(a: &State, t: &str, order: &str, id: &str) -> Result<Value> {
    let mut v: Value = sqlx::query_scalar(
        "SELECT snapshot FROM order_receipts WHERE tenant=$1 AND order_id=$2 AND id=$3",
    )
    .bind(t)
    .bind(order)
    .bind(id)
    .fetch_optional(&a.db)
    .await?
    .ok_or(Error(
        StatusCode::NOT_FOUND,
        "Issued receipt not found for this order".into(),
    ))?;
    if let Some(cart) = v["order"]["cart"].as_object_mut() {
        cart.remove("token");
    }
    Ok(v)
}
