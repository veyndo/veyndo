//! Flow execution summaries; the reader's scopes decide how much of each job it sees.
use crate::store::flow_job_summaries;
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::domain::permissions::{SCOPES, allowed};
use veyndo_identity::{merchant, permit};

/// The tenant's flow jobs; full details only for a reader holding every scope.
pub async fn values(a: &State, t: &str, h: &RequestContext) -> Result<Value> {
    let full = SCOPES.iter().all(|s| allowed(h, s));
    flow_job_summaries::values(a, t, full).await
}

/// `GET /api/automation/executions`.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "automation.read")?;
    Ok(Json(json!({"jobs":values(&a,&merchant(&h)?,&h).await?})))
}
