//! HTTP proposal planning and the caller's reviewable task list.
use crate::domain::views::Queued;
use crate::store::tasks;
use axum::Json;
use axum::extract::State as Shared;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{RequestContext, Result, State};
use veyndo_identity::{SCOPES, allowed, assistant_access, merchant, permit};
use veyndo_intelligence::domain::run_result::plan_result;
use veyndo_intelligence::service::agent_runs::{enqueue, wait};
use veyndo_intelligence::service::models::choice;
use veyndo_shop::service::language::language_context;

/// The JSON fields `agent_plan` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AgentPlanInput {
    instruction: Value,
    workspace_context: Value,
    #[serde(deserialize_with = "lenient")]
    background: Option<bool>,
}

/// Queues a planning run and waits for its proposal, or returns at once in the background.
pub async fn agent_plan(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: AgentPlanInput = view(&v);
    assistant_access(&h)?;
    merchant(&h)?;
    permit(&h, "knowledge.read")?;
    let selection = choice(&v)?;
    let queued=enqueue(&a,&h,json!({"instruction":input.instruction,"workspaceContext":input.workspace_context,"inference":selection,"locale":language_context(&a,&h).await?.0,"channel":channel_id(&h)}),None).await?;
    if input.background == Some(true) {
        return Ok(Json(queued));
    }
    let run = wait(&a, &h, &view::<Queued>(&queued).task_id.unwrap()).await?;
    Ok(Json(plan_result(run)?))
}
/// The caller's recent reviewable tasks that their current rights still cover.
pub async fn tasks(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    assistant_access(&h)?;
    let t = merchant(&h)?;
    let rs = tasks::reviewable(
        &a,
        &t,
        header(&h, "x-rac-user"),
        &json!(SCOPES.iter().filter(|s| allowed(&h, s)).collect::<Vec<_>>()),
    )
    .await?;
    Ok(Json(
        json!({"tasks":rs.iter().map(|r|json!({"id":r.id,"preview":r.proposal,"applied":r.applied,"executionStarted":r.execution_started})).collect::<Vec<_>>()}),
    ))
}
