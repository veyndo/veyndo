//! The process-owned state of the app platform: the bounded policy compilation cache and the per-service execution limits.
use crate::service::limits::ServiceLimits;
use crate::service::sandbox_cache::Cache;
use std::sync::Arc;

/// Shared by every request of this process; cloning shares the same cache and limits.
#[derive(Clone, Default)]
pub struct AppRuntime {
    /// Compiled business policy modules by tenant and digest.
    pub sandboxes: Arc<Cache>,
    /// Concurrent external service calls per tenant and app.
    pub limits: Arc<ServiceLimits>,
}
