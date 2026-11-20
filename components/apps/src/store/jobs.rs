//! Durable long actions piggyback on the existing outbox. Apps claim a fenced lease; unknown side effects are never retried automatically.
use crate::domain::{approval, persistence};
use serde_json::Value;
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::shop_scope::{header, parent};
use veyndo_core::{RequestContext, Result, State, bad, uid};
use veyndo_identity::merchant;

/// Queues one long action of a personal account in a live shop under an idempotency key.
pub async fn enqueue(
    a: &State,
    h: &RequestContext,
    m: &Manifest,
    action: &Action,
    v: &Value,
) -> Result<Value> {
    let t = merchant(h)?;
    if parent(a, &t).await?.is_some() {
        return Err(bad("Long external jobs require a live workspace"));
    }
    let actor = h
        .principal
        .user
        .as_deref()
        .filter(|u| *u != "bootstrap")
        .ok_or(bad("Personal account required for long jobs"))?;
    let key = header(h, "idempotency-key")
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Idempotency-Key is required for a long app job"))?;
    if v.to_string().len() > 16384 {
        return Err(bad("App job input exceeds 16 KiB"));
    }
    let package_digest = approval::canonical_digest(m);
    let id = uid();
    let event = event_kind(m, &action.name);
    let mut tx = a.db.begin().await?;
    let result = veyndo_app_storage::jobs::enqueue(
        &mut tx,
        &veyndo_app_storage::jobs::NewJob {
            scope: veyndo_app_storage::Scope {
                tenant: &t,
                app: &m.id,
            },
            id: &id,
            action: &action.name,
            actor,
            permission: action.permission.as_deref().unwrap_or("apps.manage"),
            package_digest: &package_digest,
            request_key: key,
            input: v,
            event_kind: &event,
        },
    )
    .await
    .map_err(persistence::error)?;
    tx.commit().await?;
    Ok(result)
}
/// The outbox event kind of a job action.
pub fn event_kind(m: &Manifest, action: &str) -> String {
    veyndo_app_storage::job_projection::event_kind(m, action)
}
