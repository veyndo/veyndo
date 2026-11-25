//! SQL of approved work execution: lease-fenced task checkpoints, operation admission and translation draft steps.
use super::staging_documents::Tx;
use serde_json::Value;
use sqlx::postgres::PgRow;
use veyndo_core::{Result, State};

/// Marks the operation the leased run is about to perform; returns the rows changed.
pub async fn mark_pending(a: &State, t: &str, id: &str, lease: &str, key: &str) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_pending=$4,run_revision=run_revision+1 WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()", t, id, lease, key).execute(&a.db).await?.rows_affected())
}

/// Locks the task while the given lease still runs it; `false` when the lease is lost.
pub async fn lock_leased(tx: &mut Tx<'_>, t: &str, id: &str, lease: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!("SELECT id FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() FOR UPDATE", t, id, lease).fetch_optional(&mut **tx).await?.is_some())
}

/// Stores an admission token for a native operation under the lease; returns the rows changed.
pub async fn admit_operation(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    admission: &str,
    token: &Value,
    lease: &str,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$5 AND run_until>now()", t, id, admission, token, lease).execute(&mut **tx).await?.rows_affected())
}

/// The completion of one operation and the lease or admission that fences it.
pub struct Receipt<'a> {
    pub lease: &'a str,
    pub key: &'a str,
    pub receipt: Value,
    pub admission: &'a str,
    pub native: bool,
}

/// Records an operation receipt under its lease or retained admission; returns the rows changed.
pub async fn record_receipt(tx: &mut Tx<'_>, t: &str, id: &str, r: Receipt<'_>) -> Result<u64> {
    Ok(sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint-$6::text,ARRAY[$4],$5,true),run_pending=CASE WHEN run_state='running' THEN NULL ELSE run_pending END,run_revision=run_revision+1,run_updated=now(),run_until=CASE WHEN run_state='running' AND run_until>now() THEN now()+interval '240 seconds' ELSE run_until END WHERE tenant=$1 AND id=$2 AND ((run_lease=$3 AND run_state='running' AND run_until>now()) OR ($7 AND run_checkpoint->$6=jsonb_build_object('lease',$3::text) AND ((run_state IN ('cancelled','uncertain') AND run_lease IS NULL) OR (run_state='running' AND run_lease=$3))))", t, id, r.lease, r.key, r.receipt, r.admission, r.native).execute(&mut **tx).await?.rows_affected())
}

/// Completes the leased run; `None` when the lease is lost, else the task's conversation.
pub async fn complete(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    lease: &str,
) -> Result<Option<Option<String>>> {
    Ok(sqlx::query_scalar!("UPDATE tasks SET applied=true,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() RETURNING conversation_id", t, id, lease).fetch_optional(&mut **tx).await?)
}

/// Releases the conversation turn the task holds.
pub async fn release_turn(tx: &mut Tx<'_>, t: &str, conversation: &str, task: &str) -> Result<()> {
    sqlx::query!("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3", t, conversation, task).execute(&mut **tx).await?;
    Ok(())
}

/// The full row of a queued or running work task, decoded by the identity authority mapper.
pub async fn work_task(a: &State, t: &str, task: &str) -> Result<Option<PgRow>> {
    // dynamic SQL: the whole row is decoded by identity's shared `agent_authority::headers(&PgRow)`.
    Ok(sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 AND run_state IN ('queued','running') AND run_request ? 'execution'")
        .bind(t).bind(task).fetch_optional(&a.db).await?)
}

/// The state of a translation job a work step started.
pub struct StepJob {
    pub status: String,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

/// Locks the translation job a work step started.
pub async fn step_job(tx: &mut Tx<'_>, t: &str, job: &str, task: &str) -> Result<Option<StepJob>> {
    Ok(sqlx::query_as!(StepJob, "SELECT status,error,error_code FROM translation_jobs WHERE tenant=$1 AND id=$2 AND run_task=$3 FOR UPDATE", t, job, task).fetch_optional(&mut **tx).await?)
}

/// Marks a step as pending without touching the lease.
pub async fn set_pending(tx: &mut Tx<'_>, t: &str, task: &str, pending: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE tasks SET run_pending=$3 WHERE tenant=$1 AND id=$2",
        t,
        task,
        pending
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Links a translation job to the work task that started it.
pub async fn link_job(tx: &mut Tx<'_>, t: &str, job: &str, task: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE translation_jobs SET run_task=$3 WHERE tenant=$1 AND id=$2",
        t,
        job,
        task
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Stores one checkpoint entry without touching the lease.
pub async fn checkpoint(
    tx: &mut Tx<'_>,
    t: &str,
    task: &str,
    key: &str,
    value: &Value,
) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2", t, task, key, value).execute(&mut **tx).await?;
    Ok(())
}

/// Records a step receipt and extends the lease.
pub async fn record_step(
    tx: &mut Tx<'_>,
    t: &str,
    task: &str,
    key: &str,
    receipt: &Value,
    lease: &str,
) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true),run_revision=run_revision+1,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$5", t, task, key, receipt, lease).execute(&mut **tx).await?;
    Ok(())
}

/// Yields the worker: requeues the task for five seconds and drops the lease.
pub async fn yield_run(tx: &mut Tx<'_>, t: &str, task: &str, lease: &str) -> Result<()> {
    sqlx::query!("UPDATE tasks SET run_state='queued',run_after=now()+interval '5 seconds',run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$3", t, task, lease).execute(&mut **tx).await?;
    Ok(())
}
