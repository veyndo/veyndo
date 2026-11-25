//! Durable agent runs extend native tasks: admission with quota and replay detection, revision-bound status, cancel and explicit resume. Leases fence checkpoints and publication, never approved writes.
use super::agent_controls::conversation_actor;
use super::models::choice;
use crate::store::agent_runs::{claim_conversation, project};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_cognition::inference::{InferenceError, budget};
use veyndo_core::admission::reserve_ai_attempt_tx;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash, uid};
use veyndo_identity::{SCOPES, allowed, assistant_access, merchant};
use veyndo_shop::service::language::language_context;

/// The run a worker executes right now.
#[derive(Clone)]
pub struct Run {
    /// The run's shop.
    pub tenant: String,
    /// The task ID.
    pub id: String,
    /// The worker's lease on the task.
    pub lease: String,
    /// The saved presentation context.
    pub workspace: Value,
    /// Whether the run answers a conversation turn.
    pub conversation: bool,
}
tokio::task_local! {
    /// The run of the current worker task.
    pub static CURRENT: Run;
}
/// The ID of the run being executed, if any.
pub fn current_id() -> Option<String> {
    CURRENT.try_with(|r| r.id.clone()).ok()
}
/// The saved presentation context of the run being executed.
pub fn current_context() -> Value {
    CURRENT
        .try_with(|run| run.workspace.clone())
        .unwrap_or(Value::Null)
}

/// Checks a bounded presentation context; it never supplies identity, rights or trusted object facts.
pub fn normalize(value: &Value) -> Result<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    validate_native(&context_schema(), value)?;
    Ok(value.clone())
}
/// The schema of the presentation context saved with a run.
pub fn context_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["view"],"properties":{
        "view":{"type":"string","enum":["assistant","workspace","setup","overview","knowledge","agents","commerce","design","productData","products","orders","customers","b2b","pos","storyfronts","channels","automation","apps","developers","environments","users"]},
        "selected":{"type":"object","additionalProperties":false,"required":["kind","id"],"properties":{"kind":{"type":"string","enum":["product","order","customer","company","app","page","flow"]},"id":{"type":"string","minLength":1,"maxLength":160}}}
    }})
}
/// The run operations.
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "merchant.run.create",
        "Queue a durable native planning task; no automatic business writes",
    ),
    (
        "merchant.run.status",
        "Read a saved run and completed proposal with current rights",
    ),
    (
        "merchant.run.cancel",
        "Cancel a saved run with its expected revision",
    ),
    (
        "merchant.run.resume",
        "Explicitly resume a failed or ambiguous run from completed checkpoints",
    ),
];
/// The input schema of a run operation.
pub fn schema(name: &str) -> Option<Value> {
    CAPABILITIES.iter().any(|(n,_)|*n==name).then(||json!({"type":"object","properties":{"requestId":{"type":"string","minLength":1,"maxLength":64},"id":{"type":"string","maxLength":64},"revision":{"type":"integer","minimum":1},"approve":{"type":"boolean"},"instruction":{"type":"string","minLength":1,"maxLength":4000},"inference":{"type":"object","properties":{"provider":{"type":"string","enum":["platform","ollama","openai","anthropic"]},"model":{"type":["string","null"],"maxLength":128}},"required":["provider"],"additionalProperties":false}},"required":if name=="merchant.run.create" {vec!["instruction"]}else if name=="merchant.run.status" {vec!["id"]}else{vec!["id","revision","approve"]},"additionalProperties":false})).map(|mut schema| { schema["properties"]["workspaceContext"] = context_schema(); schema })
}
/// Queues a run after admission: shop agents live, request ID replay, backlog, conversation turn and the shared AI quota.
pub async fn enqueue(
    a: &State,
    h: &RequestContext,
    mut request: Value,
    conversation: Option<&str>,
) -> Result<Value> {
    assistant_access(h)?;
    let t = merchant(h)?;
    let mut admission = RunAdmission {
        workspace_context: normalize(request.get("workspaceContext").unwrap_or(&Value::Null))?,
        agent_id: h.principal.agent_key.clone(),
        inference: None,
    };
    let requested = choice(&request)?;
    if h.principal.agent_key.is_some() {
        admission.inference = Some(json!(
            budget::choice(Some(&a.db), requested.as_ref())
                .await
                .map_err(|e| Error::from(InferenceError::Admission(e)))?
        ));
    }
    merge(&mut request, admission)?;
    let input: RunRequest = view(&request);
    let instruction = input
        .instruction
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4000)
        .ok_or(bad("Instruction must contain 1..4000 characters"))?;
    if request.to_string().len() > 32768 || h.principal.app.is_some() {
        return Err(bad("Invalid durable agent request"));
    }
    let actor = header(h, "x-rac-user").ok_or(bad("Agent actor required"))?;
    let credential = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(hash);
    if actor != "bootstrap" && credential.is_none() {
        return Err(bad("Current credential required"));
    }
    let id = uid();
    let scopes = SCOPES
        .iter()
        .filter(|s| allowed(h, s))
        .copied()
        .collect::<Vec<_>>();
    // Quota shares the live shop across sandboxes. One admitted native transaction
    // commits both quota and work without acquiring a second pooler backend.
    let mut tx = a.db.begin_privileged().await?;
    let stopped: bool = veyndo_core::store::tenants::agents_stopped_locked(&mut *tx, &t).await?;
    if stopped {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shop agents are stopped by an administrator".into(),
        ));
    }
    if !h.principal.personal_session && actor != "bootstrap" && h.principal.agent_key.is_none() {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Use a dedicated agent key with explicit budgets".into(),
        ));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,2))")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    let request_key = request
        .get("requestId")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 64
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                })
                .ok_or(bad("Invalid agent request ID"))
        })
        .transpose()?;
    let mut fingerprint_input = vec![
        input.instruction.clone(),
        input.inference,
        input.locale,
        input.channel,
        input.requested_conversation,
        input.workspace_context,
    ];
    if input.scenario.is_object() {
        fingerprint_input.push(input.scenario);
    }
    let fingerprint_input = json!(fingerprint_input);
    let fingerprint = hash(&fingerprint_input.to_string());
    if let Some(key)=request_key && let Some(existing)=sqlx::query("SELECT *,run_updated::text AS updated_text FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_key=$3 AND (run_request->>'agentId') IS NOT DISTINCT FROM $4").bind(&t).bind(actor).bind(key).bind(&h.principal.agent_key).fetch_optional(&mut *tx).await? {
        if existing.get::<Option<String>,_>("run_fingerprint").as_deref()!=Some(&fingerprint) {return Err(conflict("Agent request ID was already used with different input"));}
        let admitted:Value=existing.get("run_scopes");
        if admitted.as_array().is_none_or(|s|s.iter().any(|s|s.as_str().is_none_or(|s|!allowed(h,s)))) {return Err(Error(StatusCode::FORBIDDEN,"Saved request evidence requires its original permissions".into()));}
        return Ok(project(&existing));
    }
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM tasks WHERE tenant=$1 AND run_state IN ('queued','running','uncertain')").bind(&t).fetch_one(&mut *tx).await?;
    if count >= 20 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Agent backlog limit reached; finish or cancel a saved run".into(),
        ));
    }
    if input.new_conversation == Some(true)
        && let Some(c) = conversation
    {
        sqlx::query("INSERT INTO conversations(tenant,id,title,owner_id) VALUES($1,$2,$3,$4)")
            .bind(&t)
            .bind(c)
            .bind(instruction.chars().take(70).collect::<String>())
            .bind(conversation_actor(h))
            .execute(&mut *tx)
            .await?;
    }
    if let Some(c) = conversation {
        let free:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversations WHERE tenant=$1 AND id=$2 AND (turn_until IS NULL OR turn_until<now()) AND NOT EXISTS(SELECT 1 FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND run_state IN ('queued','running','uncertain'))) FOR UPDATE").bind(&t).bind(c).fetch_one(&mut *tx).await?;
        if !free {
            return Err(conflict(
                "A saved turn is already active in this conversation",
            ));
        }
        sqlx::query("INSERT INTO chat_messages(tenant,conversation_id,role,content,data) VALUES($1,$2,'user',$3,$4)").bind(&t).bind(c).bind(instruction).bind(json!({"runId":id})).execute(&mut *tx).await?;
    }
    // Only new work consumes the shared budget. Replays above remain recoverable at the daily limit.
    reserve_ai_attempt_tx(a, &t, &mut tx).await?;
    sqlx::query("INSERT INTO tasks(tenant,id,proposal,run_state,run_request,run_actor,run_scopes,run_credential,run_personal,conversation_id,run_key,run_fingerprint) VALUES($1,$2,'{}','queued',$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(&t).bind(&id).bind(&request).bind(actor).bind(json!(scopes)).bind(credential).bind(h.principal.personal_session).bind(conversation).bind(request_key).bind(&fingerprint).execute(&mut *tx).await?;
    if let Some(c) = conversation {
        sqlx::query("UPDATE conversations SET turn_owner=$3,turn_until='infinity' WHERE tenant=$1 AND id=$2").bind(&t).bind(c).bind(&id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    sqlx::query("SELECT pg_notify('veyndo_work_ready','')")
        .execute(&a.db)
        .await?;
    Ok(
        json!({"taskId":id,"agentId":h.principal.agent_key,"state":"queued","revision":1,"approvalRequired":true,"conversationId":conversation}),
    )
}

/// A durable run request as it is stored and fingerprinted.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RunRequest {
    instruction: Value,
    inference: Value,
    locale: Value,
    channel: Value,
    requested_conversation: Value,
    workspace_context: Value,
    scenario: Value,
    #[serde(deserialize_with = "lenient")]
    new_conversation: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    agent_id: Option<String>,
}

/// What admission adds to a run request: its normalized context, the admitting agent and the budgeted model.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunAdmission {
    workspace_context: Value,
    agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inference: Option<Value>,
}

/// The input of a run operation: create, status, cancel or resume.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RunOperation {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    instruction: Value,
    request_id: Value,
    workspace_context: Value,
}

/// Reads, cancels or explicitly resumes a saved run with current rights and revision.
pub async fn operate(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let schema = schema(name).ok_or(bad("Unknown agent run operation"))?;
    validate_native(&schema, v)?;
    assistant_access(h)?;
    let t = merchant(h)?;
    let input: RunOperation = view(v);
    if name == "merchant.run.create" {
        return enqueue(a,h,json!({"instruction":input.instruction,"inference":v.get("inference"),"locale":language_context(a,h).await?.0,"channel":channel_id(h),"requestId":input.request_id,"workspaceContext":input.workspace_context}),None).await;
    }
    // Resume charges the shared live-shop budget in the same transaction.
    // Status/cancel retain the ordinary actor-scoped connection.
    let mut tx = if name == "merchant.run.resume" {
        a.db.begin_privileged().await?
    } else {
        a.db.begin().await?
    };
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,2))")
        .bind(&t)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT *,run_updated::text AS updated_text FROM tasks WHERE tenant=$1 AND id=$2 AND run_state IS NOT NULL FOR UPDATE",
    )
    .bind(&t)
    .bind(input.id.as_deref().ok_or(bad("Agent run ID required"))?)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Agent run not found".into()))?;
    let actor: String = row.get("run_actor");
    let request: RunRequest = view(&row.get::<Value, _>("run_request"));
    if !h.principal.personal_session
        && actor != "bootstrap"
        && (h.principal.agent_key.is_none()
            || request.agent_id.as_deref() != h.principal.agent_key.as_deref())
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Run belongs to another agent identity".into(),
        ));
    }
    if header(h, "x-rac-user") != Some(actor.as_str())
        && !(name == "merchant.run.cancel"
            && h.principal.personal_session
            && header(h, "x-rac-role") == Some("owner"))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Only the original run actor may access its continuation".into(),
        ));
    }
    let scopes: Value = row.get("run_scopes");
    if name != "merchant.run.cancel"
        && scopes
            .as_array()
            .is_none_or(|s| s.iter().any(|s| s.as_str().is_none_or(|s| !allowed(h, s))))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Run evidence requires its original permissions".into(),
        ));
    }
    let state: String = row.get("run_state");
    let revision: i64 = row.get("run_revision");
    if name == "merchant.run.status" {
        let mut value = project(&row);
        if state == "running"
            && value["conversationId"].is_string()
            && h.principal.agent_key.is_none()
            && let Some(views) = row.get::<Value, _>("run_checkpoint").get("app-views")
        {
            value["appViews"] = views.clone();
        }
        return Ok(value);
    }
    if input.approve != Some(true) || input.revision != Some(revision) {
        return Err(conflict(
            "Explicit approval and current run revision required",
        ));
    }
    let next = if name == "merchant.run.cancel" {
        if state == "ready" || state == "cancelled" {
            return Err(conflict(
                "Completed run cannot be cancelled; its proposal still needs approval",
            ));
        }
        "cancelled"
    } else {
        if !["failed", "uncertain"].contains(&state.as_str()) {
            return Err(conflict("Only a stopped run can be explicitly resumed"));
        }
        let stopped: bool = veyndo_core::store::tenants::agents_stopped(&mut *tx, &t).await?;
        if stopped {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Shop agents are stopped by an administrator".into(),
            ));
        }
        reserve_ai_attempt_tx(a, &t, &mut tx).await?;
        "queued"
    };
    if next == "queued"
        && let Some(c) = row.get::<Option<String>, _>("conversation_id")
    {
        claim_conversation(&mut tx, &t, &c, &row.get::<String, _>("id")).await?;
    }
    if next == "queued" && row.get::<bool, _>("run_personal") != h.principal.personal_session {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Use the original personal or integration credential kind to continue".into(),
        ));
    }
    let credential = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(hash);
    if actor != "bootstrap" && credential.is_none() {
        return Err(bad("Current run credential required"));
    }
    sqlx::query("UPDATE tasks SET run_state=$3,run_error=NULL,run_pending=NULL,run_lease=NULL,run_until=NULL,run_credential=$4,run_personal=$5,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2")
   .bind(&t).bind(row.get::<String,_>("id")).bind(next).bind(credential).bind(h.principal.personal_session).execute(&mut *tx).await?;
    sqlx::query("UPDATE tasks SET run_after=now() WHERE tenant=$1 AND id=$2")
        .bind(&t)
        .bind(row.get::<String, _>("id"))
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE translation_jobs SET status=$3,error=NULL,error_code=NULL,lease=NULL,lease_until=NULL WHERE tenant=$1 AND run_task=$2 AND status IN ('queued','processing','failed')")
        .bind(&t).bind(row.get::<String,_>("id")).bind(if next=="cancelled" {"cancelled"} else {"queued"}).execute(&mut *tx).await?;
    if next == "cancelled" {
        sqlx::query("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3").bind(&t).bind(row.get::<Option<String>,_>("conversation_id")).bind(row.get::<String,_>("id")).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"taskId":row.get::<String,_>("id"),"state":next,"revision":revision+1,"approvalRequired":true}),
    )
}
/// Compatibility wait is only a transport: dropping it never deletes the persisted run.
pub async fn wait(a: &State, h: &RequestContext, id: &str) -> Result<Value> {
    let start = std::time::Instant::now();
    loop {
        let value = operate(a, h, "merchant.run.status", &json!({"id":id})).await?;
        if ![Some("queued"), Some("running")].contains(&value["state"].as_str())
            || start.elapsed().as_secs() >= 185
        {
            return Ok(value);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

/// The caller's latest run in a conversation.
pub async fn conversation_run(a: &State, h: &RequestContext, conversation: &str) -> Result<Value> {
    let t = merchant(h)?;
    let id:Option<String>=sqlx::query_scalar("SELECT id FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND run_actor=$3 AND ($4::boolean OR (run_request->>'agentId') IS NOT DISTINCT FROM $5) ORDER BY created_at DESC LIMIT 1").bind(t).bind(conversation).bind(header(h,"x-rac-user")).bind(h.principal.personal_session).bind(&h.principal.agent_key).fetch_optional(&a.db).await?;
    match id {
        Some(id) => operate(a, h, "merchant.run.status", &json!({"id":id})).await,
        None => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_operations_have_closed_schemas() {
        assert_eq!(
            schema("merchant.run.create").unwrap()["required"],
            json!(["instruction"])
        );
        assert_eq!(
            schema("merchant.run.cancel").unwrap()["required"],
            json!(["id", "revision", "approve"])
        );
        assert!(schema("merchant.run.other").is_none());
        assert!(current_id().is_none());
        assert!(current_context().is_null());
    }
    #[test]
    fn setup_is_bounded_presentation_context() {
        let setup = json!({"view":"setup"});
        assert_eq!(normalize(&setup).unwrap(), setup);
        assert!(normalize(&json!({"view":"setup","tenant":"another-shop"})).is_err());
        assert!(normalize(&json!({"view":"unknown"})).is_err());
    }
}
