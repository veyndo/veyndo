//! Durable agent runs in storage: the caller projection, the conversation turn fence, publication under the run lease and the checkpoint journal.
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgExecutor, Postgres, Row, Transaction};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode, conflict};

/// The lease a worker holds on one running task.
#[derive(Clone, Copy)]
pub struct Lease<'a> {
    pub tenant: &'a str,
    pub id: &'a str,
    pub lease: &'a str,
}

/// Share one conversation turn fence between explicit continuation and approved work.
pub async fn claim_conversation(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    conversation: &str,
    task: &str,
) -> Result<()> {
    let row = sqlx::query!(
        r#"SELECT turn_owner,(turn_until IS NULL OR turn_until<now()) AS "available!" FROM conversations WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        t,
        conversation
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(conflict("Conversation unavailable"))?;
    let busy = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND id<>$3 AND run_state IN ('queued','running','uncertain')) AS "busy!""#,
        t,
        conversation,
        task
    )
    .fetch_one(&mut **tx)
    .await?;
    if busy || (row.turn_owner.as_deref() != Some(task) && !row.available) {
        return Err(conflict("A newer conversation turn is active"));
    }
    take_turn(&mut **tx, t, conversation, task).await
}

/// Gives the conversation turn to the task until it is released.
pub async fn take_turn(
    e: impl PgExecutor<'_>,
    t: &str,
    conversation: &str,
    task: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE conversations SET turn_owner=$3,turn_until='infinity' WHERE tenant=$1 AND id=$2",
        t,
        conversation,
        task
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Releases the conversation turn if the task still holds it.
pub async fn release_turn(
    e: impl PgExecutor<'_>,
    t: &str,
    conversation: Option<&str>,
    task: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3",
        t,
        conversation,
        task
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Appends a message to a conversation.
pub async fn message(
    e: impl PgExecutor<'_>,
    t: &str,
    conversation: &str,
    role: &str,
    content: &str,
    data: &Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO chat_messages(tenant,conversation_id,role,content,data) VALUES($1,$2,$3,$4,$5)",
        t,
        conversation,
        role,
        content,
        data
    )
    .execute(e)
    .await?;
    Ok(())
}

/// A saved run as its owner and the caller projection read it.
pub struct TaskRun {
    pub id: String,
    pub proposal: Value,
    pub applied: bool,
    pub run_state: String,
    pub run_request: Value,
    pub run_actor: String,
    pub run_fingerprint: Option<String>,
    pub run_scopes: Value,
    pub run_personal: bool,
    pub run_checkpoint: Value,
    pub run_revision: i64,
    pub run_error: Option<Value>,
    pub conversation_id: Option<String>,
    pub updated_text: String,
}

/// Serializes run admission and continuation within a shop.
pub async fn lock_shop(e: impl PgExecutor<'_>, t: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,2))", t)
        .execute(e)
        .await?;
    Ok(())
}

/// The run an actor already started with this request ID under this agent identity.
pub async fn replay(
    e: impl PgExecutor<'_>,
    t: &str,
    actor: &str,
    key: &str,
    agent: Option<&str>,
) -> Result<Option<TaskRun>> {
    Ok(sqlx::query_as!(
        TaskRun,
        r#"SELECT id,proposal,applied,run_state AS "run_state!",run_request AS "run_request!",run_actor AS "run_actor!",run_fingerprint,run_scopes AS "run_scopes!",run_personal,run_checkpoint,run_revision,run_error,conversation_id,run_updated::text AS "updated_text!" FROM tasks WHERE tenant=$1 AND run_actor=$2 AND run_key=$3 AND (run_request->>'agentId') IS NOT DISTINCT FROM $4"#,
        t,
        actor,
        key,
        agent
    )
    .fetch_optional(e)
    .await?)
}

/// A saved run, row-locked for a status read or a state change.
pub async fn locked(e: impl PgExecutor<'_>, t: &str, id: &str) -> Result<Option<TaskRun>> {
    Ok(sqlx::query_as!(
        TaskRun,
        r#"SELECT id,proposal,applied,run_state AS "run_state!",run_request AS "run_request!",run_actor AS "run_actor!",run_fingerprint,run_scopes AS "run_scopes!",run_personal,run_checkpoint,run_revision,run_error,conversation_id,run_updated::text AS "updated_text!" FROM tasks WHERE tenant=$1 AND id=$2 AND run_state IS NOT NULL FOR UPDATE"#,
        t,
        id
    )
    .fetch_optional(e)
    .await?)
}

/// Runs of the shop that still occupy the backlog.
pub async fn backlog(e: impl PgExecutor<'_>, t: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM tasks WHERE tenant=$1 AND run_state IN ('queued','running','uncertain')"#,
        t
    )
    .fetch_one(e)
    .await?)
}

/// Starts a conversation.
pub async fn create_conversation(
    e: impl PgExecutor<'_>,
    t: &str,
    id: &str,
    title: &str,
    owner: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO conversations(tenant,id,title,owner_id) VALUES($1,$2,$3,$4)",
        t,
        id,
        title,
        owner
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Whether the conversation has no active turn and no active run, row-locked.
pub async fn conversation_free(
    e: impl PgExecutor<'_>,
    t: &str,
    conversation: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM conversations WHERE tenant=$1 AND id=$2 AND (turn_until IS NULL OR turn_until<now()) AND NOT EXISTS(SELECT 1 FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND run_state IN ('queued','running','uncertain'))) AS "free!" FOR UPDATE"#,
        t,
        conversation
    )
    .fetch_one(e)
    .await?)
}

/// A run as admission stores it.
pub struct NewRun<'a> {
    pub id: &'a str,
    pub request: &'a Value,
    pub actor: &'a str,
    pub scopes: &'a Value,
    pub credential: Option<&'a str>,
    pub personal: bool,
    pub conversation: Option<&'a str>,
    pub key: Option<&'a str>,
    pub fingerprint: &'a str,
}

/// Queues a new run.
pub async fn insert(e: impl PgExecutor<'_>, t: &str, run: NewRun<'_>) -> Result<()> {
    sqlx::query!(
        "INSERT INTO tasks(tenant,id,proposal,run_state,run_request,run_actor,run_scopes,run_credential,run_personal,conversation_id,run_key,run_fingerprint) VALUES($1,$2,'{}','queued',$3,$4,$5,$6,$7,$8,$9,$10)",
        t,
        run.id,
        run.request,
        run.actor,
        run.scopes,
        run.credential,
        run.personal,
        run.conversation,
        run.key,
        run.fingerprint
    )
    .execute(e)
    .await?;
    Ok(())
}

/// Wakes the workers.
pub async fn notify_ready(e: impl PgExecutor<'_>) -> Result<()> {
    sqlx::query!("SELECT pg_notify('veyndo_work_ready','')")
        .execute(e)
        .await?;
    Ok(())
}

/// Moves a run to `state` (queued or cancelled) under the current credential, due now, together with its translation jobs.
pub async fn transition(
    tx: &mut PgConnection,
    t: &str,
    id: &str,
    state: &str,
    credential: Option<&str>,
    personal: bool,
) -> Result<()> {
    sqlx::query!(
        "UPDATE tasks SET run_state=$3,run_error=NULL,run_pending=NULL,run_lease=NULL,run_until=NULL,run_credential=$4,run_personal=$5,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2",
        t,
        id,
        state,
        credential,
        personal
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE tasks SET run_after=now() WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE translation_jobs SET status=$3,error=NULL,error_code=NULL,lease=NULL,lease_until=NULL WHERE tenant=$1 AND run_task=$2 AND status IN ('queued','processing','failed')",
        t,
        id,
        if state == "cancelled" { "cancelled" } else { "queued" }
    )
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// The caller's latest run in a conversation; personal sessions see runs of every agent identity.
pub async fn latest_in_conversation(
    e: impl PgExecutor<'_>,
    t: &str,
    conversation: &str,
    actor: Option<&str>,
    personal: bool,
    agent: Option<&str>,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND run_actor=$3 AND ($4::boolean OR (run_request->>'agentId') IS NOT DISTINCT FROM $5) ORDER BY created_at DESC LIMIT 1",
        t,
        conversation,
        actor,
        personal,
        agent
    )
    .fetch_optional(e)
    .await?)
}
/// The fields of a run request the caller projection shows.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ProjectedRequest {
    #[serde(deserialize_with = "lenient")]
    execution: Option<Vec<Value>>,
    channel: Value,
    agent_id: Value,
}

/// The name and label of one planned operation.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlanStep {
    name: Value,
    label: Value,
}

/// The scenario progress saved in a run checkpoint.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ScenarioCheckpoint {
    #[serde(rename = "scenario-state", deserialize_with = "lenient")]
    scenario_state: ScenarioSteps,
}

/// The steps of a scenario run.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ScenarioSteps {
    steps: Value,
}

/// What a caller may see of a run; checkpoints and credentials stay inside.
pub fn project(row: &TaskRun) -> Value {
    let state = &row.run_state;
    let cp = &row.run_checkpoint;
    let request = &row.run_request;
    let shown = view::<ProjectedRequest>(request);
    let execution=shown.execution.as_ref().map(|plan|json!({"total":plan.len(),"completed":cp.as_object().map_or(0,|o|o.keys().filter(|k|k.starts_with("operation-")).count()),"steps":plan.iter().enumerate().map(|(n,s)|{let s=view::<PlanStep>(s);json!({"name":s.name,"label":s.label,"completed":cp.get(format!("operation-{n}")).is_some(),"translationJobId":cp.get(format!("translation-{n}"))})}).collect::<Vec<_>>()}));
    let steps = cp
        .as_object()
        .map_or(0, |o| o.keys().filter(|k| k.starts_with("model-")).count());
    let scenario = request.get("scenario").map(|s| {
        let mut result = s.clone();
        result["steps"] = view::<ScenarioCheckpoint>(cp).scenario_state.steps;
        result
    });
    json!({"taskId":row.id,"agentId":shown.agent_id,"channel":shown.channel,"state":state,"revision":row.run_revision,"completedModelSteps":steps,"scenario":scenario,"execution":execution,"error":row.run_error,"conversationId":row.conversation_id,"approvalRequired":!row.applied,"preview":if state=="ready" {row.proposal.clone()}else{Value::Null},"updatedAt":row.updated_text})
}

/// The published evidence of an agent run.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunEvidence {
    #[serde(deserialize_with = "lenient")]
    proposal: ProposalSummary,
}

/// The summary of a proposal.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProposalSummary {
    #[serde(deserialize_with = "lenient")]
    summary: Option<String>,
}

/// Publishes the finished proposal while the lease is still held, and releases the conversation turn.
pub async fn publish(a: &State, t: &str, id: &str, evidence: &Value, lease: &str) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let row = sqlx::query!(
        "UPDATE tasks SET proposal=$1,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$2 AND id=$3 AND run_state='running' AND run_lease=$4 AND run_until>now() RETURNING conversation_id",
        evidence,
        t,
        id,
        lease
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(conflict("Agent run cancelled or lease lost"))?;
    if let Some(c) = row.conversation_id {
        release_turn(&mut *tx, t, Some(&c), id).await?;
        let data = json!({"taskId":id,"preview":evidence,"approvalRequired":true});
        let summary = view::<RunEvidence>(evidence).proposal.summary;
        message(
            &mut *tx,
            t,
            &c,
            "assistant",
            summary.as_deref().unwrap_or(""),
            &data,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// One saved checkpoint of a running task.
pub async fn load(a: &State, r: Lease<'_>, key: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT run_checkpoint->$4 FROM tasks WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()",
        r.tenant,
        r.id,
        r.lease,
        key
    )
    .fetch_optional(&a.db)
    .await?
    .flatten())
}

/// Saves a checkpoint and extends the lease; a lost lease is a conflict.
pub async fn save(a: &State, r: Lease<'_>, key: &str, value: &Value) -> Result<()> {
    let changed = sqlx::query!(
        "UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$4],$5,true),run_pending=NULL,run_revision=run_revision+1,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()",
        r.tenant,
        r.id,
        r.lease,
        key,
        value
    )
    .execute(&a.db)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(conflict("Agent run cancelled or lease lost"));
    }
    Ok(())
}

/// Marks an inference call as pending before the provider is called, so an ambiguous outcome stays visible.
pub async fn pending(a: &State, r: Lease<'_>, key: &str) -> Result<()> {
    let changed = sqlx::query!(
        "UPDATE tasks SET run_pending=$4,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()",
        r.tenant,
        r.id,
        r.lease,
        key
    )
    .execute(&a.db)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(conflict("Agent lease lost before inference"));
    }
    Ok(())
}

/// Requeues expired runs; a run with a pending provider call becomes uncertain instead, since its outcome is unknown.
pub async fn recover(a: &State) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_state=CASE WHEN run_pending IS NULL THEN 'queued' ELSE 'uncertain' END,run_error=CASE WHEN run_pending IS NULL THEN NULL ELSE jsonb_build_object('code',CASE WHEN run_request ? 'execution' THEN 'agent_work_interrupted' ELSE 'agent_model_interrupted' END,'retryable',false) END,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE (tenant,id) IN (SELECT tenant,id FROM tasks WHERE run_state='running' AND run_until<now() ORDER BY run_updated LIMIT 20 FOR UPDATE SKIP LOCKED)").execute(&a.db).await?;
    Ok(())
}
/// Claims the next queued run under a new lease: one ordinary run or two scenario runs per shop at a time.
pub async fn claim(a: &State, lease: &str) -> Result<Option<PgRow>> {
    let mut tx = a.db.begin().await?;
    // runtime SQL: the whole row goes to veyndo_identity agent_authority::headers, which reads a PgRow.
    let row=sqlx::query("SELECT * FROM tasks r WHERE run_state='queued' AND run_after<=now() AND NOT EXISTS(SELECT 1 FROM tasks active WHERE active.tenant=r.tenant AND active.run_state='running' AND active.run_until>now() AND NOT(active.run_request ? 'scenario')) AND (SELECT count(*) FROM tasks active WHERE active.tenant=r.tenant AND active.run_state='running' AND active.run_until>now()) < CASE WHEN r.run_request ? 'scenario' THEN 2 ELSE 1 END ORDER BY run_updated,tenant,id FOR UPDATE SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let t: String = row.get("tenant");
    let id: String = row.get("id");
    // Serialize tenant admission across replicas, then recheck the running count.
    let admitted = sqlx::query_scalar!(
        r#"SELECT pg_try_advisory_xact_lock(hashtextextended($1,2)) AS "admitted!""#,
        t
    )
    .fetch_one(&mut *tx)
    .await?;
    if !admitted {
        return Ok(None);
    };
    let is_scenario = view::<RunKind>(&row.get::<Value, _>("run_request"))
        .scenario
        .is_object();
    let busy = sqlx::query_scalar!(
        r#"SELECT count(*) >= $2 OR coalesce(bool_or(NOT(run_request ? 'scenario')),false) AS "busy!" FROM tasks WHERE tenant=$1 AND run_state='running' AND run_until>now()"#,
        t,
        if is_scenario { 2_i64 } else { 1_i64 }
    )
    .fetch_one(&mut *tx)
    .await?;
    if busy {
        return Ok(None);
    };
    sqlx::query!("UPDATE tasks SET run_state='running',run_checkpoint=run_checkpoint-'app-views',run_lease=$3,run_until=now()+interval '240 seconds',run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2",
        t, id, lease).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(row))
}
/// Whether a run request is a scenario run.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunKind {
    scenario: Value,
}

/// The problem code a failed run records.
fn problem_code(error: &Error) -> &'static str {
    match error.0 {
        StatusCode::FORBIDDEN => "agent_authority_changed",
        StatusCode::TOO_MANY_REQUESTS => "agent_capacity_busy",
        _ => "agent_step_failed",
    }
}

/// The actor-private problem a failed run records; outbox projections carry IDs and states only.
pub fn problem(error: &Error) -> Value {
    let code = problem_code(error);
    json!({"code":code,"status":error.0.as_u16(),"detail":error.1.chars().take(500).collect::<String>()})
}
/// Stops a failed run under its lease, tells its conversation and releases a finished turn.
pub async fn fail(a: &State, r: Lease<'_>, error: &Error) -> Result<()> {
    let problem = problem(error);
    let mut tx = a.db.begin().await?;
    let stopped = sqlx::query!(
        r#"UPDATE tasks SET run_state=CASE WHEN run_pending IS NOT NULL THEN 'uncertain' ELSE 'failed' END,run_error=$4,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now() RETURNING conversation_id,run_state AS "run_state!""#,
        r.tenant,
        r.id,
        r.lease,
        &problem
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(stopped) = stopped
        && let Some(c) = stopped.conversation_id
    {
        let state = stopped.run_state;
        let data = json!({"error":true,"runId":r.id,"runState":state,"code":problem_code(error),"providerUnavailable":error.0==StatusCode::BAD_GATEWAY});
        let detail = error.1.chars().take(500).collect::<String>();
        message(&mut *tx, r.tenant, &c, "assistant", &detail, &data).await?;
        if state == "failed" {
            release_turn(&mut *tx, r.tenant, Some(&c), r.id).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// The running task under this lease, row-locked inside a commerce transaction; the lease must hold at statement time.
pub async fn running_locked(tx: &mut PgConnection, r: Lease<'_>) -> Result<Option<PgRow>> {
    // runtime SQL: the whole row goes to veyndo_identity agent_authority::headers, which reads a PgRow.
    Ok(sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>statement_timestamp() FOR UPDATE")
        .bind(r.tenant).bind(r.id).bind(r.lease).fetch_optional(tx).await?)
}

/// The running task under this lease.
pub async fn running(a: &State, r: Lease<'_>) -> Result<Option<PgRow>> {
    // runtime SQL: the whole row goes to veyndo_identity agent_authority::headers, which reads a PgRow.
    Ok(sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()")
        .bind(r.tenant).bind(r.id).bind(r.lease).fetch_optional(&a.db).await?)
}

/// The transaction's `rac.system` setting.
pub async fn system_setting(tx: &mut PgConnection) -> Result<String> {
    Ok(
        sqlx::query_scalar!(r#"SELECT current_setting('rac.system',true) AS "setting!""#)
            .fetch_one(tx)
            .await?,
    )
}

/// Sets the transaction's `rac.system` setting.
pub async fn set_system_setting(tx: &mut PgConnection, value: &str) -> Result<()> {
    sqlx::query_scalar!("SELECT set_config('rac.system',$1,true)", value)
        .fetch_one(tx)
        .await?;
    Ok(())
}

/// Whether the task's lease still holds at statement time.
pub async fn lease_current(tx: &mut PgConnection, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT run_until>statement_timestamp() AS "current!" FROM tasks WHERE tenant=$1 AND id=$2"#,
        t,
        id
    )
    .fetch_one(tx)
    .await?)
}

/// Marks a scenario step as pending under the run lease; false when the lease is lost.
pub async fn step_pending(a: &State, r: Lease<'_>, step: &str) -> Result<bool> {
    Ok(sqlx::query!("UPDATE tasks SET run_pending=$4 WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()",
        r.tenant, r.id, r.lease, step).execute(&a.db).await?.rows_affected() == 1)
}

/// Completes a scenario run with its report as applied evidence under the lease; false when the lease is lost.
pub async fn complete_scenario(a: &State, r: Lease<'_>, report: &Value) -> Result<bool> {
    Ok(sqlx::query!("UPDATE tasks SET proposal=$4,applied=true,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()",
        r.tenant, r.id, r.lease, report).execute(&a.db).await?.rows_affected() == 1)
}
