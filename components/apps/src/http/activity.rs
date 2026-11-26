//! Bounded metadata-only app telemetry, storage usage and explicitly approved cursor-based event replay.
use crate::domain::event_projection;
use crate::store::packages::package;
use crate::store::{activity, locks};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::merchant;

/// The latest calls, deliveries and storage use of an app; metadata only.
pub async fn activity(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    package(&a, &t, &id, false).await?;
    let calls = activity::calls(&a, &t, &id).await?;
    let deliveries = activity::deliveries(&a, &t, &id).await?;
    let usage = activity::usage(&a, &t, &id).await?;
    Ok(Json(
        json!({"calls":calls,"deliveries":deliveries,"storage":usage,"delivery":"at-least-once","retentionCalls":1000}),
    ))
}
/// The JSON fields `replay` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReplayInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    after: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    limit: Option<i64>,
}

/// Queues eligible outbox events after a cursor again; the caller approves repeated external effects.
pub async fn replay(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: ReplayInput = view(&v);
    let t = merchant(&h)?;
    if input.approve != Some(true) {
        return Err(bad(
            "Approve event replay; external effects may occur again",
        ));
    }
    if parent(&a, &t).await?.is_some() {
        return Err(bad("Private sandbox events cannot be delivered externally"));
    }
    let m = package(&a, &t, &id, true).await?;
    let after = input
        .after
        .filter(|n| *n >= 0)
        .ok_or(bad("Nonnegative event cursor required"))?;
    let limit = if v.get("limit").is_none() {
        50
    } else {
        input
            .limit
            .filter(|n| (1..=100).contains(n))
            .ok_or(bad("Replay limit must be 1..100"))?
    };
    let mut tx = a.db.begin().await?;
    locks::advisory(&mut tx, &format!("{t}:{id}"), 843).await?;
    let rows = activity::events_after(&mut tx, &t, after, limit).await?;
    let mut queued = 0;
    for r in &rows {
        if event_projection::eligible(&m, &r.kind, &r.data) {
            queued += activity::requeue(&mut tx, &t, &id, r.id).await?;
        }
    }
    veyndo_core::store::outbox::publish_on(
        &mut *tx,
        &t,
        "app.replay_requested",
        &json!({"app":id,"after":after,"queued":queued,"actor":h.principal.user}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"queued":queued,"nextCursor":rows.last().map(|r|r.id).unwrap_or(after),"hasMore":rows.len()==limit as usize}),
    ))
}
