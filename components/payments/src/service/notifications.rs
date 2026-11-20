//! Version-pinned HMAC notifications enqueue reconciliation; external event payloads never write monetary state.
use crate::ports::Host;
use crate::store::attempts::attempt;
use crate::store::jobs::enqueue_tx;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, hash};

/// Verifies a signed provider notification of one tenant and queues a reconciliation once per event.
pub async fn accept(
    host: &dyn Host,
    a: &State,
    h: &RequestContext,
    t: &str,
    provider: &str,
    bytes: &[u8],
) -> Result<Value> {
    let v: Value =
        serde_json::from_slice(bytes).map_err(|_| bad("Invalid provider notification"))?;
    let id = v["attemptId"].as_str().ok_or(bad("Attempt required"))?;
    let row =
        sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2 AND provider=$3")
            .bind(t)
            .bind(id)
            .bind(provider)
            .fetch_optional(&a.db)
            .await?
            .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    let p = attempt(&row);
    let c = host.service(&p.provider, &p.adapter_version, &p.environment)?;
    let stamp = header(h, "x-payment-timestamp")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or(bad("Notification timestamp required"))?;
    if chrono::Utc::now().timestamp().abs_diff(stamp) > 300 {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Notification expired".into(),
        ));
    }
    let signature = header(h, "x-payment-signature")
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(bad("Notification signature required"))?;
    let raw = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&signature[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| bad("Invalid signature encoding"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(c["token"].as_str().unwrap().as_bytes())
        .map_err(|_| bad("Invalid notification key"))?;
    mac.update(stamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(bytes);
    mac.verify_slice(&raw).map_err(|_| {
        Error(
            StatusCode::UNAUTHORIZED,
            "Invalid provider signature".into(),
        )
    })?;
    if v["apiVersion"] != "1"
        || v["tenant"] != t
        || v["provider"] != p.provider
        || v["adapterVersion"] != p.adapter_version
        || v["environment"] != p.environment
        || v["accountRef"] != p.context["accountRef"]
        || v["reference"].as_str() != p.provider_order.as_deref()
    {
        return Err(bad("Provider event identity mismatch"));
    }
    let event = v["eventId"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Event id required"))?;
    let mut tx = a.db.begin().await?;
    let count=sqlx::query("INSERT INTO payment_inbox(tenant,provider,event_id,data) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(t).bind(&p.provider).bind(event).bind(json!({"attemptId":id,"reference":p.provider_order})).execute(&mut *tx).await?.rows_affected();
    if count == 1 {
        enqueue_tx(
            &mut tx,
            t,
            id,
            "reconcile",
            &format!("notify:{}", &hash(&format!("{}:{event}", p.provider))[..40]),
            &json!({}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(json!({"accepted":true,"duplicate":count==0}))
}
