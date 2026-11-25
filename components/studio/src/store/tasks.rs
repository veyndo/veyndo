//! SQL of merchant proposals and reviewed work on the shared `tasks` table: review lists, approval locks and execution queueing.
use super::staging_documents::Tx;
use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::PgRow;
use veyndo_core::{Result, State};

/// Whether the task's proposal contains native operations (reviewed work).
pub async fn has_operations(a: &State, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT coalesce(jsonb_array_length(proposal->'proposal'->'operations'),0)>0 FROM tasks WHERE tenant=$1 AND id=$2").bind(t).bind(id).fetch_optional(&a.db).await?.unwrap_or(false))
}

/// The proposal evidence of a task that is not applied yet.
pub async fn unapplied_proposal(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(
        sqlx::query_scalar("SELECT proposal FROM tasks WHERE tenant=$1 AND id=$2 AND NOT applied")
            .bind(t)
            .bind(id)
            .fetch_optional(&a.db)
            .await?,
    )
}

/// The whole task row, locked.
pub async fn lock(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Option<PgRow>> {
    Ok(
        sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(t)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?,
    )
}

/// Marks the task's proposal as applied.
pub async fn mark_applied(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    sqlx::query("UPDATE tasks SET applied=true,applied_at=now() WHERE tenant=$2 AND id=$1")
        .bind(id)
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The whole task row.
pub async fn get(a: &State, t: &str, id: &str) -> Result<PgRow> {
    Ok(sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2")
        .bind(t)
        .bind(id)
        .fetch_one(&a.db)
        .await?)
}

/// The task's run revision, locked.
pub async fn lock_revision(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT run_revision FROM tasks WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(t)
            .bind(id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

/// Queues the approved work for execution with the approving credential.
pub async fn queue_execution(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    request: &Value,
    credential: Option<String>,
    personal: bool,
) -> Result<()> {
    sqlx::query("UPDATE tasks SET run_request=$3,run_credential=$4,run_personal=$5,run_checkpoint='{}',run_state='queued',run_revision=run_revision+1,run_updated=now(),run_lease=NULL,run_until=NULL WHERE tenant=$1 AND id=$2").bind(t).bind(id).bind(request).bind(credential).bind(personal).execute(&mut **tx).await?;
    Ok(())
}

/// One reviewable task of an actor.
pub struct Reviewable {
    pub id: String,
    pub proposal: Value,
    pub applied: bool,
    pub execution_started: bool,
}

/// The actor's 30 most recent reviewable tasks whose scopes the given rights cover.
pub async fn reviewable(
    a: &State,
    t: &str,
    actor: Option<&str>,
    scopes: &Value,
) -> Result<Vec<Reviewable>> {
    let rs = sqlx::query(
        "SELECT id,proposal,applied,coalesce(run_request ? 'execution',false) AS execution_started FROM tasks WHERE tenant=$1 AND (run_state IS NULL OR run_state='ready' OR run_request ? 'execution') AND coalesce(run_actor,proposal->>'actor')=$2 AND (run_scopes IS NULL OR run_scopes <@ $3) ORDER BY created_at DESC LIMIT 30",
    )
    .bind(t)
    .bind(actor)
    .bind(scopes)
    .fetch_all(&a.db)
    .await?;
    Ok(rs
        .iter()
        .map(|r| Reviewable {
            id: r.get("id"),
            proposal: r.get("proposal"),
            applied: r.get("applied"),
            execution_started: r.get("execution_started"),
        })
        .collect())
}

/// The proposal and scopes of the actor's completed, applied work in a live or owned preview workspace.
pub async fn completed_work(
    tx: &mut Tx<'_>,
    t: &str,
    task: &str,
    actor: &str,
) -> Result<Option<(Value, Value)>> {
    Ok(sqlx::query("SELECT task.proposal,task.run_scopes FROM tasks task JOIN shop_environments e ON e.tenant=task.tenant WHERE task.tenant=$1 AND task.id=$2 AND task.run_actor=$3 AND task.applied AND task.run_state='ready' AND task.run_request ? 'execution' AND (e.preview_owner IS NULL OR (e.preview_owner=$3 AND e.preview_until>now()))")
        .bind(t).bind(task).bind(actor).fetch_optional(&mut **tx).await?
        .map(|row| (row.get("proposal"), row.get("run_scopes"))))
}
