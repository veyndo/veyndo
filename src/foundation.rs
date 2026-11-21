//! Application dependencies, error responses and request context helpers.
use crate::*;

#[derive(Clone)]
pub(crate) struct App {
    /// Database pool and HTTP client; component crates take only this part.
    pub(crate) core: veyndo_core::State,
    pub(crate) _connection_budget: Arc<performance::cluster_lease::Lease>,
    pub(crate) inference_slots: Arc<tokio::sync::Semaphore>,
    pub(crate) token: Arc<String>,
    pub(crate) inference: Inference,
    pub(crate) model: Arc<String>,
    pub(crate) sandboxes: Arc<sandbox_cache::Cache>,
    pub(crate) channel_metrics: Arc<channel_metrics::ChannelMetrics>,
    pub(crate) app_limits: Arc<apps::ServiceLimits>,
}
impl std::ops::Deref for App {
    type Target = veyndo_core::State;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}
/// Component routers take only the core part of the server state.
impl axum::extract::FromRef<App> for veyndo_core::State {
    fn from_ref(app: &App) -> Self {
        app.core.clone()
    }
}
pub(crate) use veyndo_core::shop_scope::{header, tenant, token};
pub(crate) use veyndo_core::{Error, Result, bad, conflict, hash, uid, validate_tenant};
pub(crate) fn merchant(_a: &App, h: &RequestContext) -> Result<String> {
    veyndo_identity::merchant(h)
}
