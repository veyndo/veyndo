//! Private queue age diagnostics using the existing ready indexes.
use std::time::Duration;
use veyndo::scoped_pool::ScopedPool;

pub(super) async fn report(db: &ScopedPool) {
    let read = crate::store::process::oldest_due(db);
    match tokio::time::timeout(Duration::from_secs(3), read).await {
        Ok(Ok((flow, delivery))) => tracing::info!(
            flow_due = flow.is_some(),
            flow_oldest_seconds = flow.unwrap_or(0),
            app_delivery_due = delivery.is_some(),
            app_delivery_oldest_seconds = delivery.unwrap_or(0),
            "app_worker_queue_sample"
        ),
        Ok(Err(_)) => tracing::warn!(reason = "database_error", "app_worker_queue_unavailable"),
        Err(_) => tracing::warn!(reason = "timeout", "app_worker_queue_unavailable"),
    }
}
