//! Operator-only per-shop daily interactive AI limits, optimistic revisions and durable audit.
use super::operator::actor;
use crate::store::operator_control;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, validate_tenant};
use veyndo_kernel::verified_kernel;

/// The daily interactive AI limit of a shop with its revision and the use of today.
pub async fn get(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    actor(&h)?;
    validate_tenant(&id)?;
    let exists: bool = veyndo_core::store::tenants::exists(&a.db, &id).await?;
    if !exists {
        return Err(Error(StatusCode::NOT_FOUND, "Shop not found".into()));
    }
    let r = operator_control::limit(&a, &id).await?;
    let usage = operator_control::used_today(&a, &id).await?;
    Ok(Json(
        json!({"tenant":id,"dailyAi":r.as_ref().map(|r|r.daily_ai),"revision":r.map(|r|r.revision).unwrap_or(1),"usedToday":usage.unwrap_or(0),"scope":"database / UTC day; interactive HTTP AI attempts","defaults":a.admission.snapshot()}),
    ))
}
/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    #[serde(deserialize_with = "lenient")]
    daily_ai: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<u64>,
}

/// Changes the limit at the expected revision and records the change in the audit trail.
pub async fn save(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SaveInput = view(&v);
    let actor = actor(&h)?;
    validate_tenant(&id)?;
    let daily = input
        .daily_ai
        .filter(|n| verified_kernel::resource_quota_admissible(*n))
        .ok_or(bad("dailyAi must be 1..1000000"))? as i64;
    let revision = input.revision.ok_or(bad("Revision required"))?;
    let mut tx = a.db.begin().await?;
    // Tenant row serializes first insert and every update without a missing-row race.
    veyndo_core::store::tenants::lock_if_exists(&mut *tx, &id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Shop not found".into()))?;
    let current = operator_control::limit_revision(&mut tx, &id).await?;
    if !verified_kernel::revision_admissible(current.unwrap_or(1) as u64, revision) {
        return Err(conflict("Quota revision changed"));
    }
    operator_control::upsert_limit(&mut tx, &id, daily, revision as i64 + 1).await?;
    operator_control::audit(
        &mut tx,
        actor,
        "shop.quota_changed",
        &id,
        &json!({"dailyAi":daily,"revision":revision+1}),
    )
    .await?;
    tx.commit().await?;
    get(State(a), h, Path(id)).await
}
