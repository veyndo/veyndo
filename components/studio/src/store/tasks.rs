//! SQL of merchant proposals and reviewed work on the shared `tasks` table: review lists, approval locks and execution queueing.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::{Result, State};
pub use veyndo_intelligence::store::work_tasks::ProposalTask;
pub use veyndo_intelligence::store::work_tasks::Reviewable;
pub use veyndo_intelligence::store::work_tasks::WorkTask;

/// Whether the task's proposal contains native operations (reviewed work).
pub async fn has_operations(a: &State, t: &str, id: &str) -> Result<bool> {
    veyndo_intelligence::store::work_tasks::has_operations(a, t, id).await
}

/// The proposal evidence of a task that is not applied yet.
pub async fn unapplied_proposal(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    veyndo_intelligence::store::work_tasks::unapplied_proposal(a, t, id).await
}

/// The task's approval state, locked.
pub async fn lock(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Option<ProposalTask>> {
    veyndo_intelligence::store::work_tasks::lock_proposal(tx, t, id).await
}

/// Marks the task's proposal as applied.
pub async fn mark_applied(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    veyndo_intelligence::store::work_tasks::mark_applied(tx, t, id).await
}

/// The task's approval state.
pub async fn get(a: &State, t: &str, id: &str) -> Result<WorkTask> {
    veyndo_intelligence::store::work_tasks::reviewed_work(a, t, id).await
}

/// The task's run revision, locked.
pub async fn lock_revision(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<i64> {
    veyndo_intelligence::store::work_tasks::lock_revision(tx, t, id).await
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
    veyndo_intelligence::store::work_tasks::queue_execution(
        tx, t, id, request, credential, personal,
    )
    .await
}

/// The actor's 30 most recent reviewable tasks whose scopes the given rights cover.
pub async fn reviewable(
    a: &State,
    t: &str,
    actor: Option<&str>,
    scopes: &Value,
) -> Result<Vec<Reviewable>> {
    veyndo_intelligence::store::work_tasks::reviewable(a, t, actor, scopes).await
}

/// The proposal and scopes of the actor's completed, applied work in a live or owned preview workspace.
pub async fn completed_work(
    tx: &mut Tx<'_>,
    t: &str,
    task: &str,
    actor: &str,
) -> Result<Option<(Value, Option<Value>)>> {
    veyndo_intelligence::store::work_tasks::completed_work(tx, t, task, actor).await
}
