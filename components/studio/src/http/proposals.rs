//! HTTP proposal planning and the caller's reviewable task list.
use axum::Json;
use axum::extract::State as Shared;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{SCOPES, allowed, assistant_access, merchant, permit};
use veyndo_intelligence::domain::run_result::plan_result;
use veyndo_intelligence::service::agent_runs::{enqueue, wait};
use veyndo_intelligence::service::models::choice;
use veyndo_shop::service::language::language_context;

/// Queues a planning run and waits for its proposal, or returns at once in the background.
pub async fn agent_plan(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    assistant_access(&h)?;
    merchant(&h)?;
    permit(&h, "knowledge.read")?;
    let selection = choice(&v)?;
    let queued=enqueue(&a,&h,json!({"instruction":v["instruction"],"workspaceContext":v["workspaceContext"],"inference":selection,"locale":language_context(&a,&h).await?.0,"channel":channel_id(&h)}),None).await?;
    if v["background"] == true {
        return Ok(Json(queued));
    }
    let run = wait(&a, &h, queued["taskId"].as_str().unwrap()).await?;
    Ok(Json(plan_result(run)?))
}
/// The caller's recent reviewable tasks that their current rights still cover.
pub async fn tasks(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    assistant_access(&h)?;
    let t = merchant(&h)?;
    let rs = sqlx::query(
        "SELECT id,proposal,applied,coalesce(run_request ? 'execution',false) AS execution_started FROM tasks WHERE tenant=$1 AND (run_state IS NULL OR run_state='ready' OR run_request ? 'execution') AND coalesce(run_actor,proposal->>'actor')=$2 AND (run_scopes IS NULL OR run_scopes <@ $3) ORDER BY created_at DESC LIMIT 30",
    )
    .bind(t)
    .bind(header(&h,"x-rac-user"))
    .bind(json!(SCOPES.iter().filter(|s|allowed(&h,s)).collect::<Vec<_>>()))
    .fetch_all(&a.db)
    .await?;
    Ok(Json(
        json!({"tasks":rs.iter().map(|r|json!({"id":r.get::<String,_>("id"),"preview":r.get::<Value,_>("proposal"),"applied":r.get::<bool,_>("applied"),"executionStarted":r.get::<bool,_>("execution_started")})).collect::<Vec<_>>()}),
    ))
}
