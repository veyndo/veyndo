//! Reviewed work and proposal tasks: proposal reads and application marks, queued execution, leased work-run checkpoints and receipts, turn release and the wake-up of tasks waiting on translation jobs.
use serde_json::Value;
use sqlx::postgres::PgRow;
use veyndo_core::{Result, State};

/// One reviewable task of an actor.
pub struct Reviewable {
    pub id: String,
    pub proposal: Value,
    pub applied: bool,
    pub execution_started: bool,
}

/// What approving reviewed work reads of its task; work tasks always carry their actor, scopes and request.
pub struct WorkTask {
    pub applied: bool,
    pub run_actor: String,
    pub run_scopes: Value,
    pub run_request: Value,
    pub run_state: Option<String>,
    pub proposal: Value,
    pub run_revision: i64,
    pub conversation_id: Option<String>,
}

/// What approving a proposal reads of its task.
pub struct ProposalTask {
    pub run_state: Option<String>,
    pub proposal: Value,
    pub run_actor: Option<String>,
    pub run_scopes: Option<Value>,
    pub applied: bool,
}

/// The completion of one operation and the lease or admission that fences it.
pub struct Receipt<'a> {
    pub lease: &'a str,
    pub key: &'a str,
    pub receipt: Value,
    pub admission: &'a str,
    pub native: bool,
}

/// Wakes the queued parent task once all of its translation jobs have stopped.
pub async fn wake_task(a: &State, t: &str, task: &str) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_after=now() WHERE tenant=$1 AND id=$2 AND run_state='queued' AND EXISTS(SELECT 1 FROM translation_jobs j WHERE j.tenant=$1 AND j.run_task=$2 AND j.status IN ('ready','failed','cancelled'))", t, task).execute(&a.db).await?;
    Ok(())
}

/// Marks the operation the leased run is about to perform; returns the rows changed.
pub async fn mark_pending(a: &State, t: &str, id: &str, lease: &str, key: &str) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_pending=$4,run_revision=run_revision+1 WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()", t, id, lease, key).execute(&a.db).await?.rows_affected())
}

/// Locks the task while the given lease still runs it; `false` when the lease is lost.
pub async fn lock_leased(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    lease: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!("SELECT id FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() FOR UPDATE", t, id, lease).fetch_optional(&mut *c).await?.is_some())
}

/// Stores an admission token for a native operation under the lease; returns the rows changed.
pub async fn admit_operation(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    admission: &str,
    token: &Value,
    lease: &str,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$5 AND run_until>now()", t, id, admission, token, lease).execute(&mut *c).await?.rows_affected())
}

/// Records an operation receipt under its lease or retained admission; returns the rows changed.
pub async fn record_receipt(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    r: Receipt<'_>,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint-$6::text,ARRAY[$4],$5,true),run_pending=CASE WHEN run_state='running' THEN NULL ELSE run_pending END,run_revision=run_revision+1,run_updated=now(),run_until=CASE WHEN run_state='running' AND run_until>now() THEN now()+interval '240 seconds' ELSE run_until END WHERE tenant=$1 AND id=$2 AND ((run_lease=$3 AND run_state='running' AND run_until>now()) OR ($7 AND run_checkpoint->$6=jsonb_build_object('lease',$3::text) AND ((run_state IN ('cancelled','uncertain') AND run_lease IS NULL) OR (run_state='running' AND run_lease=$3))))", t, id, r.lease, r.key, r.receipt, r.admission, r.native).execute(&mut *c).await?.rows_affected())
}

/// Completes the leased run; `None` when the lease is lost, else the task's conversation.
pub async fn complete(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    lease: &str,
) -> Result<Option<Option<String>>> {
    Ok(sqlx::query_scalar!("UPDATE tasks SET applied=true,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() RETURNING conversation_id", t, id, lease).fetch_optional(&mut *c).await?)
}

/// Releases the conversation turn the task holds.
pub async fn release_turn(
    c: &mut sqlx::PgConnection,
    t: &str,
    conversation: &str,
    task: &str,
) -> Result<()> {
    sqlx::query!("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3", t, conversation, task).execute(&mut *c).await?;
    Ok(())
}

/// The full row of a queued or running work task, decoded by the identity authority mapper.
pub async fn work_task(a: &State, t: &str, task: &str) -> Result<Option<PgRow>> {
    // dynamic SQL: the whole row is decoded by identity's shared `agent_authority::headers(&PgRow)`.
    Ok(sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 AND run_state IN ('queued','running') AND run_request ? 'execution'")
        .bind(t).bind(task).fetch_optional(&a.db).await?)
}

/// Marks a step as pending without touching the lease.
pub async fn set_pending(
    c: &mut sqlx::PgConnection,
    t: &str,
    task: &str,
    pending: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE tasks SET run_pending=$3 WHERE tenant=$1 AND id=$2",
        t,
        task,
        pending
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Stores one checkpoint entry without touching the lease.
pub async fn checkpoint(
    c: &mut sqlx::PgConnection,
    t: &str,
    task: &str,
    key: &str,
    value: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2", t, task, key, value).execute(&mut *c).await?;
    Ok(())
}

/// Records a step receipt and extends the lease.
pub async fn record_step(
    c: &mut sqlx::PgConnection,
    t: &str,
    task: &str,
    key: &str,
    receipt: &Value,
    lease: &str,
) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true),run_revision=run_revision+1,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$5", t, task, key, receipt, lease).execute(&mut *c).await?;
    Ok(())
}

/// Yields the worker: requeues the task for five seconds and drops the lease.
pub async fn yield_run(c: &mut sqlx::PgConnection, t: &str, task: &str, lease: &str) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_state='queued',run_after=now()+interval '5 seconds',run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$3", t, task, lease).execute(&mut *c).await?;
    Ok(())
}

/// Whether the task's proposal contains native operations (reviewed work).
pub async fn has_operations(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT coalesce(jsonb_array_length(proposal->'proposal'->'operations'),0)>0 AS "work!" FROM tasks WHERE tenant=$1 AND id=$2"#, t, id).fetch_optional(&a.db).await?.unwrap_or(false))
}

/// The proposal evidence of a task that is not applied yet.
pub async fn unapplied_proposal(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT proposal FROM tasks WHERE tenant=$1 AND id=$2 AND NOT applied",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The task's approval state, locked.
pub async fn lock_proposal(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
) -> Result<Option<ProposalTask>> {
    Ok(sqlx::query_as!(
        ProposalTask,
        "SELECT run_state,proposal,run_actor,run_scopes,applied FROM tasks WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t,
        id
    )
    .fetch_optional(&mut *c)
    .await?)
}

/// Marks the task's proposal as applied.
pub async fn mark_applied(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE tasks SET applied=true,applied_at=now() WHERE tenant=$2 AND id=$1",
        id,
        t
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// Appends the applied action to a proposal explanation; proposals saved before explanations existed keep their evidence unchanged.
pub async fn explain_applied(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    action: &Value,
) -> Result<()> {
    sqlx::query!(
        "UPDATE tasks SET proposal=jsonb_set(proposal,'{explanation,actions}',coalesce(proposal#>'{explanation,actions}','[]'::jsonb)||jsonb_build_array($3::jsonb)) WHERE tenant=$1 AND id=$2 AND jsonb_typeof(proposal->'explanation')='object'",
        t,
        id,
        action
    )
    .execute(&mut *c)
    .await?;
    Ok(())
}

/// The task's approval state.
pub async fn reviewed_work(a: &State, t: &str, id: &str) -> Result<WorkTask> {
    Ok(sqlx::query_as!(
        WorkTask,
        r#"SELECT applied,run_actor AS "run_actor!",run_scopes AS "run_scopes!",run_request AS "run_request!",run_state,proposal,run_revision,conversation_id FROM tasks WHERE tenant=$1 AND id=$2"#,
        t,
        id
    )
    .fetch_one(&a.db)
    .await?)
}

/// The task's run revision, locked.
pub async fn lock_revision(c: &mut sqlx::PgConnection, t: &str, id: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        "SELECT run_revision FROM tasks WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t,
        id
    )
    .fetch_one(&mut *c)
    .await?)
}

/// Queues the approved work for execution with the approving credential.
pub async fn queue_execution(
    c: &mut sqlx::PgConnection,
    t: &str,
    id: &str,
    request: &Value,
    credential: Option<String>,
    personal: bool,
) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_request=$3,run_credential=$4,run_personal=$5,run_checkpoint='{}',run_state='queued',run_revision=run_revision+1,run_updated=now(),run_lease=NULL,run_until=NULL WHERE tenant=$1 AND id=$2", t, id, request, credential, personal).execute(&mut *c).await?;
    Ok(())
}

/// The actor's 30 most recent reviewable tasks whose scopes the given rights cover.
pub async fn reviewable(
    a: &State,
    t: &str,
    actor: Option<&str>,
    scopes: &Value,
) -> Result<Vec<Reviewable>> {
    Ok(sqlx::query_as!(
        Reviewable,
        r#"SELECT id,proposal,applied,coalesce(run_request ? 'execution',false) AS "execution_started!" FROM tasks WHERE tenant=$1 AND (run_state IS NULL OR run_state='ready' OR run_request ? 'execution') AND coalesce(run_actor,proposal->>'actor')=$2 AND (run_scopes IS NULL OR run_scopes <@ $3) ORDER BY created_at DESC LIMIT 30"#,
        t,
        actor,
        scopes
    )
    .fetch_all(&a.db)
    .await?)
}

/// The proposal and scopes of the actor's completed, applied work in a live or owned preview workspace.
pub async fn completed_work(
    c: &mut sqlx::PgConnection,
    t: &str,
    task: &str,
    actor: &str,
) -> Result<Option<(Value, Option<Value>)>> {
    Ok(sqlx::query!("SELECT task.proposal,task.run_scopes FROM tasks task JOIN shop_environments e ON e.tenant=task.tenant WHERE task.tenant=$1 AND task.id=$2 AND task.run_actor=$3 AND task.applied AND task.run_state='ready' AND task.run_request ? 'execution' AND (e.preview_owner IS NULL OR (e.preview_owner=$3 AND e.preview_until>now()))", t, task, actor)
        .fetch_optional(&mut *c)
        .await?
        .map(|row| (row.proposal, row.run_scopes)))
}
