//! Durable at-least-once app events: retry leases, stable idempotency keys and signed public HTTPS delivery on the existing outbox.
use crate::domain::event_batch::batch_size;
use crate::domain::event_destination::destination;
use crate::domain::{approval, event_projection, event_signature};
use crate::ports::Platform;
use crate::service::{admission, egress, service_call};
use crate::store::packages::package;
use crate::store::secrets;
use serde_json::{Value, json};
use sqlx::Row;
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
    let row=sqlx::query("WITH candidate AS (SELECT d.tenant,d.app,d.event_id FROM app_deliveries d JOIN app_packages p ON p.tenant=d.tenant AND p.id=d.app AND p.active JOIN tenants owner ON owner.id=d.tenant AND owner.status='active' WHERE NOT EXISTS(SELECT 1 FROM shop_environments s WHERE s.tenant=d.tenant) AND (d.app=ANY($1) OR p.manifest#>>'{eventDelivery,url}' IS NOT NULL) AND d.available_at<=now() AND (d.state='queued' OR d.state='running' AND d.lease_until<now()) AND (SELECT count(DISTINCT lease_token) FROM app_deliveries busy WHERE busy.tenant=d.tenant AND busy.state='running' AND busy.lease_until>now())<2 ORDER BY (SELECT max(last_attempt_at) FROM app_deliveries previous WHERE previous.tenant=d.tenant AND previous.app=d.app) NULLS FIRST,d.event_id LIMIT 1 FOR UPDATE OF d,owner SKIP LOCKED) UPDATE app_deliveries d SET state='running',attempts=attempts+1,lease_token=$2,last_attempt_at=now(),lease_until=now()+interval '30 seconds' FROM candidate c WHERE d.tenant=c.tenant AND d.app=c.app AND d.event_id=c.event_id RETURNING d.tenant,d.app,d.event_id").bind(ids).bind(lease).fetch_optional(&mut *tx).await?;
    let Some(r) = row else {
        return Ok(false);
    };
    let t = r.get::<String, _>("tenant");
    let app = r.get::<String, _>("app");
    let manifest: Value = sqlx::query_scalar(
        "SELECT manifest FROM app_packages WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
    )
    .bind(&t)
    .bind(&app)
    .fetch_one(&mut *tx)
    .await?;
    let m: Manifest = serde_json::from_value(manifest).map_err(|_| bad("Invalid event package"))?;
    let extra = batch_size(&m) - 1;
    if extra > 0 {
        sqlx::query("WITH candidates AS (SELECT tenant,app,event_id FROM app_deliveries WHERE tenant=$1 AND app=$2 AND available_at<=now() AND (state='queued' OR state='running' AND lease_until<now()) ORDER BY event_id LIMIT $4 FOR UPDATE SKIP LOCKED) UPDATE app_deliveries d SET state='running',attempts=attempts+1,lease_token=$3,last_attempt_at=now(),lease_until=now()+interval '30 seconds' FROM candidates c WHERE d.tenant=c.tenant AND d.app=c.app AND d.event_id=c.event_id").bind(&t).bind(&app).bind(lease).bind(extra).execute(&mut *tx).await?;
    }
    let rows=sqlx::query("SELECT o.id,o.kind,o.data,o.trace_id,d.attempts FROM app_deliveries d JOIN outbox o ON o.tenant=d.tenant AND o.id=d.event_id WHERE d.tenant=$1 AND d.app=$2 AND d.lease_token=$3 ORDER BY o.id").bind(&t).bind(&app).bind(lease).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let mut items = vec![];
    for event in &rows {
        let id = event.get::<i64, _>("id");
        let kind = event.get::<String, _>("kind");
        let data = event.get::<Value, _>("data");
        if event_projection::eligible(&m, &kind, &data) {
            items.push(json!({"idempotencyKey":format!("{t}:{app}:{id}"),"eventId":id,"tenant":t,"kind":kind,"data":event_projection::payload(&m,&kind,data)}));
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
        send(a, &t, &m, &payload).instrument(tracing::info_span!("app_event_delivery",trace_id=%rows[0].get::<String,_>("trace_id"),event_id=rows[0].get::<i64,_>("id"),app=%app)).await
    };
    let state = if outcome.is_ok() {
        "delivered"
    } else {
        "queued"
    };
    let error = outcome.err().map(|e| e.1);
    for row in &rows {
        let attempt = row.get::<i32, _>("attempts");
        let delay = (1i64 << attempt.min(8)).min(300);
        sqlx::query("UPDATE app_deliveries SET state=$1,error=$2,lease_until=NULL,lease_token=NULL,available_at=now()+make_interval(secs=>$7) WHERE tenant=$3 AND app=$4 AND event_id=$5 AND lease_token=$6").bind(if error.is_some()&&attempt>=8{"failed"}else{state}).bind(&error).bind(&t).bind(&app).bind(row.get::<i64,_>("id")).bind(lease).bind(delay as f64).execute(&a.db).await?;
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
