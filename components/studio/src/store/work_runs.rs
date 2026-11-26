//! SQL of approved work execution: lease-fenced task checkpoints, operation admission and translation draft steps.
use super::staging_documents::Tx;
use serde_json::Value;
use sqlx::postgres::PgRow;
pub use veyndo_catalog::store::translation_runs::StepJob;
use veyndo_core::{Result, State};
pub use veyndo_intelligence::store::work_tasks::Receipt;

/// Marks the operation the leased run is about to perform; returns the rows changed.
pub async fn mark_pending(a: &State, t: &str, id: &str, lease: &str, key: &str) -> Result<u64> {
    veyndo_intelligence::store::work_tasks::mark_pending(a, t, id, lease, key).await
}

/// Locks the task while the given lease still runs it; `false` when the lease is lost.
pub async fn lock_leased(tx: &mut Tx<'_>, t: &str, id: &str, lease: &str) -> Result<bool> {
    veyndo_intelligence::store::work_tasks::lock_leased(tx, t, id, lease).await
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
    veyndo_intelligence::store::work_tasks::admit_operation(tx, t, id, admission, token, lease)
        .await
}

/// Records an operation receipt under its lease or retained admission; returns the rows changed.
pub async fn record_receipt(tx: &mut Tx<'_>, t: &str, id: &str, r: Receipt<'_>) -> Result<u64> {
    veyndo_intelligence::store::work_tasks::record_receipt(tx, t, id, r).await
}

/// Completes the leased run; `None` when the lease is lost, else the task's conversation.
pub async fn complete(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    lease: &str,
) -> Result<Option<Option<String>>> {
    veyndo_intelligence::store::work_tasks::complete(tx, t, id, lease).await
}

/// Releases the conversation turn the task holds.
pub async fn release_turn(tx: &mut Tx<'_>, t: &str, conversation: &str, task: &str) -> Result<()> {
    veyndo_intelligence::store::work_tasks::release_turn(tx, t, conversation, task).await
}

/// The full row of a queued or running work task, decoded by the identity authority mapper.
pub async fn work_task(a: &State, t: &str, task: &str) -> Result<Option<PgRow>> {
    veyndo_intelligence::store::work_tasks::work_task(a, t, task).await
}

/// Locks the translation job a work step started.
pub async fn step_job(tx: &mut Tx<'_>, t: &str, job: &str, task: &str) -> Result<Option<StepJob>> {
    veyndo_catalog::store::translation_runs::step_job(tx, t, job, task).await
}

/// Marks a step as pending without touching the lease.
pub async fn set_pending(tx: &mut Tx<'_>, t: &str, task: &str, pending: &str) -> Result<()> {
    veyndo_intelligence::store::work_tasks::set_pending(tx, t, task, pending).await
}

/// Links a translation job to the work task that started it.
pub async fn link_job(tx: &mut Tx<'_>, t: &str, job: &str, task: &str) -> Result<()> {
    veyndo_catalog::store::translation_runs::link_job(tx, t, job, task).await
}

/// Stores one checkpoint entry without touching the lease.
pub async fn checkpoint(
    tx: &mut Tx<'_>,
    t: &str,
    task: &str,
    key: &str,
    value: &Value,
) -> Result<()> {
    veyndo_intelligence::store::work_tasks::checkpoint(tx, t, task, key, value).await
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
    veyndo_intelligence::store::work_tasks::record_step(tx, t, task, key, receipt, lease).await
}

/// Yields the worker: requeues the task for five seconds and drops the lease.
pub async fn yield_run(tx: &mut Tx<'_>, t: &str, task: &str, lease: &str) -> Result<()> {
    veyndo_intelligence::store::work_tasks::yield_run(tx, t, task, lease).await
}
