//! A customer test can reference only its actor's completed, applied native proposal in this sandbox.
use super::super::agent_runs::operate;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::State;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, view};
use veyndo_core::hash;
use veyndo_core::shop_scope::channel as channel_id;

pub async fn resolve(a: &State, h: &RequestContext, id: &str) -> Result<Value> {
    let run = Box::pin(operate(a, h, "merchant.run.status", &json!({"id":id}))).await?;
    let run: SourceRun = view(&run);
    if run.state != "ready" || run.approval_required != false || !run.scenario.is_null() {
        return Err(conflict(
            "Apply the completed agent proposal in this sandbox before testing it",
        ));
    }
    if run.channel.as_str().unwrap_or("default") != channel_id(h) {
        return Err(conflict(
            "Open the source proposal's sales channel before testing it",
        ));
    }
    Ok(json!({"taskId":id,"revision":run.revision,"proposalDigest":hash(&run.preview.to_string())}))
}

/// The JSON fields `recheck` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecheckInput {
    #[serde(deserialize_with = "lenient")]
    task_id: Option<String>,
}

pub async fn recheck(a: &State, h: &RequestContext, source: &Value) -> Result<()> {
    let input: RecheckInput = view(source);
    if let Some(id) = input.task_id.as_deref()
        && resolve(a, h, id).await? != *source
    {
        return Err(conflict(
            "The source proposal changed; start a new customer test",
        ));
    }
    Ok(())
}

/// The status of the proposal a customer test references.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SourceRun {
    state: Value,
    approval_required: Value,
    scenario: Value,
    channel: Value,
    revision: Value,
    preview: Value,
}
