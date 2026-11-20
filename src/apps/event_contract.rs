//! Explicit event filters, bounded batches and signed public HTTPS delivery extend the existing leased outbox, not a second queue.
use super::*;
pub(super) use veyndo_app_platform::domain::event_destination::destination;
pub(super) fn batch_size(m: &Manifest) -> i64 {
    m.event_delivery
        .as_ref()
        .map(|d| d.batch_size as i64)
        .unwrap_or(1)
}
pub(super) async fn send(a: &App, t: &str, m: &Manifest, payload: &Value) -> Result<Value> {
    let Some(url) = m.event_delivery.as_ref().and_then(|d| d.url.as_deref()) else {
        return gateway::service_call_pinned(
            a,
            t,
            &m.id,
            "events",
            payload,
            Some(&approval::canonical_digest(m)),
        )
        .await;
    };
    if staging::parent(a, t).await?.is_some() {
        return Err(bad("Outgoing events are disabled in private sandboxes"));
    }
    let current = package(a, t, &m.id, true).await?;
    if approval::canonical_digest(&current) != approval::canonical_digest(m) {
        return Err(conflict("Event package changed before dispatch"));
    }
    let _admission = service_limits::admit(a, t, &m.id).await?;
    let secret = secrets::resolve(a, t, m, "outbound").await?.ok_or(Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Set an outgoing webhook signing secret before delivery".into(),
    ))?;
    let bytes = serde_json::to_vec(payload).map_err(|_| bad("Invalid event envelope"))?;
    if bytes.len() > 65536 {
        return Err(bad("Event batch exceeds 64 KiB; reduce batchSize"));
    }
    let stamp = chrono::Utc::now().timestamp().to_string();
    let (key, signature) =
        veyndo_app_platform::domain::event_signature::sign(&secret, t, &m.id, &bytes, &stamp)?;
    let response = egress::public_client(&destination(url)?)
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
