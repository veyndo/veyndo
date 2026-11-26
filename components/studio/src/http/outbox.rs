//! Outbox runtime transport: the merchant's delivery counters and the explicit retry of a quarantined event.
use crate::store::outbox_control::{counters, release};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, shop_scope::header};
use veyndo_identity::{merchant, permit};

/// Delivery counters of the shop; instance-wide performance figures only for the instance credential.
pub async fn runtime(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let [pending, quarantined, retrying, consumed] = counters(&a, &t).await?;
    // Instance-wide diagnostic counters are visible only to the existing instance credential.
    let performance = (header(&h, "x-rac-user") == Some("bootstrap")).then(|| {
        json!({"reads":veyndo_shop::service::read_cache::reads().snapshot(),"admission":a.admission.snapshot(),"pool":{"size":a.db.size(),"idle":a.db.num_idle()}})
    });
    Ok(Json(
        json!({"outboxPending":pending,"outboxQuarantined":quarantined,"outboxRetrying":retrying,"eventsConsumed":consumed,"consumer":"independent cognition/apps/flows/audit cursors; no external messages in projection","performance":performance}),
    ))
}

/// Requeues one quarantined event of this shop; delivered or retired events cannot be replayed.
pub async fn retry(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let tenant = merchant(&h)?;
    permit(&h, "settings.write")?;
    if id <= 0 {
        return Err(bad("Positive event ID required"));
    }
    if !release(&a, &tenant, id).await? {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Quarantined event not found in this shop".into(),
        ));
    }
    Ok(Json(json!({"eventId":id,"queued":true})))
}
