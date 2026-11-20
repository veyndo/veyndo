//! The operator's own session and the control-plane audit trail.
use crate::http::operator::actor;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{RequestContext, Result};

/// Who the operator is and what the control plane lets them do.
pub async fn session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let actor = actor(&h)?;
    let row = sqlx::query("SELECT id,name,email FROM merchant_users WHERE id=$1")
        .bind(actor)
        .fetch_one(&a.db)
        .await?;
    Ok(Json(
        json!({"user":{"id":row.get::<String,_>("id"),"name":row.get::<String,_>("name"),"email":row.get::<String,_>("email")},"scope":"platform","canCreateShops":true}),
    ))
}
/// The latest hundred control-plane actions.
pub async fn audit(State(a): State<veyndo_core::State>, h: RequestContext) -> Result<Json<Value>> {
    actor(&h)?;
    let rows = sqlx::query("SELECT id,actor,action,tenant,created_at::text AS time FROM platform_audit ORDER BY id DESC LIMIT 100").fetch_all(&a.db).await?;
    Ok(Json(
        json!({"elements":rows.iter().map(|r| json!({"id":r.get::<i64,_>("id"),"actor":r.get::<Option<String>,_>("actor"),"action":r.get::<String,_>("action"),"shop":r.get::<Option<String>,_>("tenant"),"time":r.get::<String,_>("time")})).collect::<Vec<_>>()}),
    ))
}
