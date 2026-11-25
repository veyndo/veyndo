//! The operator's own session and the control-plane audit trail.
use crate::http::operator::actor;
use crate::store::operator_control;
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result};

/// Who the operator is and what the control plane lets them do.
pub async fn session(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let actor = actor(&h)?;
    let row = operator_control::account(&a, actor).await?;
    Ok(Json(
        json!({"user":{"id":row.id,"name":row.name,"email":row.email},"scope":"platform","canCreateShops":true}),
    ))
}
/// The latest hundred control-plane actions.
pub async fn audit(State(a): State<veyndo_core::State>, h: RequestContext) -> Result<Json<Value>> {
    actor(&h)?;
    let rows = operator_control::audit_trail(&a).await?;
    Ok(Json(
        json!({"elements":rows.iter().map(|r| json!({"id":r.id,"actor":r.actor,"action":r.action,"shop":r.tenant,"time":r.time})).collect::<Vec<_>>()}),
    ))
}
