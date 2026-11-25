//! SQL of the catalogue translation worker: job leases, keyset product reads and reviewed translation items.
use super::staging_documents::Tx;
pub use veyndo_catalog::store::translation_runs::Job;
pub use veyndo_catalog::store::translation_runs::NextProduct;
pub use veyndo_catalog::store::translation_runs::TranslationItem;
pub use veyndo_catalog::store::translation_runs::Window;
use veyndo_core::{Result, State};

/// Fails expired leases of task-linked jobs, which must be resumed explicitly after a possibly charged call.
pub async fn interrupt_expired(a: &State) -> Result<()> {
    veyndo_catalog::store::translation_runs::interrupt_expired(a).await
}

/// Leases the oldest runnable job of an active shop.
pub async fn lease_job(a: &State, lease: &str) -> Result<Option<Job>> {
    veyndo_catalog::store::translation_runs::lease_job(a, lease).await
}

/// Fails the leased job with an error message.
pub async fn fail(a: &State, t: &str, id: &str, lease: &str, error: &str) -> Result<()> {
    veyndo_catalog::store::translation_runs::fail(a, t, id, lease, error).await
}

/// Wakes the queued parent task once all of its translation jobs have stopped.
pub async fn wake_task(a: &State, t: &str, task: &str) -> Result<()> {
    veyndo_intelligence::store::work_tasks::wake_task(a, t, task).await
}

/// The next product after the cursor with its source and target translations.
pub async fn next_product(a: &State, t: &str, w: Window<'_>) -> Result<Option<NextProduct>> {
    veyndo_catalog::store::translation_runs::next_product(a, t, w).await
}

/// Finishes the leased job.
pub async fn finish(a: &State, t: &str, id: &str, lease: &str) -> Result<()> {
    veyndo_catalog::store::translation_runs::finish(a, t, id, lease).await
}

/// Whether the product already has a target translation with a description.
pub async fn has_target_description(
    a: &State,
    t: &str,
    product: &str,
    target: &str,
) -> Result<bool> {
    veyndo_catalog::store::translation_runs::has_target_description(a, t, product, target).await
}

/// The current lease of a processing job, locked.
pub async fn current_lease(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<Option<String>> {
    veyndo_catalog::store::translation_runs::current_lease(tx, t, id).await
}

/// Stores a translation item once and advances the job's cursor past its product.
pub async fn store_item(
    tx: &mut Tx<'_>,
    t: &str,
    id: &str,
    lease: &str,
    item: TranslationItem<'_>,
) -> Result<()> {
    veyndo_catalog::store::translation_runs::store_item(tx, t, id, lease, item).await
}
