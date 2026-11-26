//! Durable at-least-once app events: retry leases, stable idempotency keys and signed public HTTPS delivery on the existing outbox.
use crate::domain::event_batch::batch_size;
use crate::domain::event_destination::destination;
use crate::domain::{approval, event_projection, event_signature};
use crate::ports::Platform;
use crate::service::{admission, egress, service_call};
use crate::store::packages::{self, package};
use crate::store::{event_deliveries, secrets};
use serde_json::{Value, json};
use tracing::Instrument;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::store::environments::parent;
use veyndo_core::{Error, Result, StatusCode, bad, conflict, process_config};

/// Leases the next due deliveries of one app, sends them and records the outcome.
pub async fn deliver_once(a: &Platform) -> Result<bool> {
    let configured = &process_config::get().services;
    let ids = configured
        .as_object()
        .map(|v| v.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let lease = uuid::Uuid::new_v4();
    let mut tx = a.db.begin().await?;
    let row = event_deliveries::claim_next(&mut tx, &ids, lease).await?;
    let Some(r) = row else {
        return Ok(false);
    };
    let t = r.tenant;
    let app = r.app;
    let manifest = packages::active_manifest_for_share(&mut tx, &t, &app)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let m: Manifest = serde_json::from_value(manifest).map_err(|_| bad("Invalid event package"))?;
    let extra = batch_size(&m) - 1;
    if extra > 0 {
        event_deliveries::claim_more(&mut tx, &t, &app, lease, extra).await?;
    }
    let rows = event_deliveries::leased_events(&mut tx, &t, &app, lease).await?;
    tx.commit().await?;
    let mut items = vec![];
    for event in &rows {
        let id = event.id;
        let kind = &event.kind;
        let data = event.data.clone();
        if event_projection::eligible(&m, kind, &data) {
            items.push(json!({"idempotencyKey":format!("{t}:{app}:{id}"),"eventId":id,"tenant":t,"kind":kind,"data":event_projection::payload(&m,kind,data)}));
        }
    }
    let payload = if batch_size(&m) > 1 {
        json!({"apiVersion":"1","tenant":t,"app":app,"events":items})
    } else {
        items.first().cloned().unwrap_or(Value::Null)
    };
    // Unsubscribed or newly filtered deliveries complete without external effects; replay reapplies current permissions.
    let outcome = if items.is_empty() {
        Ok(json!({"skipped":true}))
    } else {
        send(a, &t, &m, &payload).instrument(tracing::info_span!("app_event_delivery",trace_id=%rows[0].trace_id,event_id=rows[0].id,app=%app)).await
    };
    let state = if outcome.is_ok() {
        "delivered"
    } else {
        "queued"
    };
    let error = outcome.err().map(|e| e.1);
    for row in &rows {
        let attempt = row.attempts;
        let delay = (1i64 << attempt.min(8)).min(300);
        event_deliveries::finish(
            a,
            &t,
            &app,
            lease,
            event_deliveries::Outcome {
                state: if error.is_some() && attempt >= 8 {
                    "failed"
                } else {
                    state
                },
                error: error.as_deref(),
                event_id: row.id,
                retry_after_seconds: delay as f64,
            },
        )
        .await?;
    }
    Ok(true)
}
/// Sends one payload to the service of an app or to its signed public webhook.
pub async fn send(a: &Platform, t: &str, m: &Manifest, payload: &Value) -> Result<Value> {
    let Some(url) = m.event_delivery.as_ref().and_then(|d| d.url.as_deref()) else {
        return service_call::service_call_pinned(
            a,
            t,
            &m.id,
            "events",
            payload,
            Some(&approval::canonical_digest(m)),
        )
        .await;
    };
    if parent(a, t).await?.is_some() {
        return Err(bad("Outgoing events are disabled in private sandboxes"));
    }
    let current = package(a, t, &m.id, true).await?;
    if approval::canonical_digest(&current) != approval::canonical_digest(m) {
        return Err(conflict("Event package changed before dispatch"));
    }
    let _admission = admission::admit(a, t, &m.id).await?;
    let secret = secrets::resolve(a, t, m, "outbound").await?.ok_or(Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Set an outgoing webhook signing secret before delivery".into(),
    ))?;
    let bytes = serde_json::to_vec(payload).map_err(|_| bad("Invalid event envelope"))?;
    if bytes.len() > 65536 {
        return Err(bad("Event batch exceeds 64 KiB; reduce batchSize"));
    }
    let stamp = chrono::Utc::now().timestamp().to_string();
    let (key, signature) = event_signature::sign(&secret, t, &m.id, &bytes, &stamp)?;
    let response = egress::public_client(
        &destination(url)?,
        &process_config::get().private_service_origins,
    )
    .await?
    .post(url)
    .header("content-type", "application/json")
    .header("x-tenant", t)
    .header("x-app-id", &m.id)
    .header("x-app-timestamp", stamp)
    .header("x-app-event-id", key)
    .header("x-app-signature", signature)
    .body(bytes)
    .send()
    .await
    .map_err(|_| {
        Error(
            StatusCode::BAD_GATEWAY,
            "Outgoing webhook unavailable".into(),
        )
    })?;
    if !response.status().is_success() {
        return Err(Error(
            StatusCode::BAD_GATEWAY,
            format!(
                "Outgoing webhook returned HTTP {}",
                response.status().as_u16()
            ),
        ));
    }
    Ok(json!({"accepted":true}))
}
