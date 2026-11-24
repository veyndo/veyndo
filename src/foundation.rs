//! Application dependencies, error responses and request context helpers.
use crate::{apps, performance, sandbox_cache};
use std::sync::Arc;
use veyndo_cognition::inference::Inference;
use veyndo_core::channel_metrics;
use veyndo_core::request_context::RequestContext;

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
impl App {
    /// The model context intelligence operations borrow: state, providers and inference slots.
    pub(crate) fn models(&self) -> veyndo_intelligence::service::models::Models<'_> {
        veyndo_intelligence::service::models::Models {
            core: &self.core,
            inference: &self.inference,
            slots: &self.inference_slots,
        }
    }
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
pub(crate) use veyndo_core::Result;
pub(crate) fn merchant(_a: &App, h: &RequestContext) -> Result<String> {
    veyndo_identity::merchant(h)
}
