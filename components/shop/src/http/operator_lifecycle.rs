//! Audited, reversible shop lifecycle; preserved commerce records and the current revision prevent accidental overwrites.
use super::operator::actor;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, validate_tenant};
use veyndo_kernel::verified_kernel;

/// Pauses, archives or restores a shop at the expected revision, with a reason in the audit trail.
pub async fn change(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let actor = actor(&h)?;
    validate_tenant(&id)?;
    let target = v["status"]
        .as_str()
        .filter(|s| ["active", "paused", "archived"].contains(s))
        .ok_or(bad("Invalid shop status"))?;
    let reason = v["reason"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 500)
        .ok_or(bad("A reason is required"))?;
    if target == "archived" && v["confirmShopId"].as_str() != Some(&id) {
        return Err(bad("Confirm the shop ID before moving it to trash"));
    }
    let mut tx = a.db.begin().await?;
    let row=sqlx::query("SELECT status,status_revision FROM tenants t WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id) FOR UPDATE").bind(&id).fetch_optional(&mut *tx).await?.ok_or(Error(StatusCode::NOT_FOUND,"Shop not found".into()))?;
    if !verified_kernel::revision_admissible(
        v["revision"]
            .as_u64()
            .filter(|r| *r > 0)
            .ok_or(bad("Positive revision required"))?,
        row.get::<i64, _>("status_revision") as u64,
    ) {
        return Err(conflict("Shop status changed; reload before editing"));
    }
    let before: String = row.get("status");
    sqlx::query("UPDATE tenants SET status=$2,status_revision=status_revision+1,status_updated_at=now() WHERE id=$1").bind(&id).bind(target).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO platform_audit(actor,action,tenant,data) VALUES($1,'shop.status_changed',$2,$3)").bind(actor).bind(&id).bind(json!({"before":before,"after":target,"reason":reason})).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"id":id,"status":target,"revision":row.get::<i64,_>("status_revision")+1,"recoverable":true}),
    ))
}
