//! Application dependencies, error responses and request context helpers.
use crate::performance;
use std::sync::Arc;
use veyndo_core::request_context::RequestContext;

/// The server's state: only the composition of component-owned state plus the process connection budget.
#[derive(Clone)]
pub(crate) struct App {
    /// Database pool, HTTP client, admission and channel metrics; component crates take only this part.
    pub(crate) core: veyndo_core::State,
    /// Inference providers, slots and the default model name (intelligence).
    pub(crate) models: veyndo_intelligence::service::models::Inferencing,
    /// Policy compilation cache and service limits (app platform).
    pub(crate) apps: veyndo_app_platform::service::runtime_state::AppRuntime,
    /// The bootstrap credential (identity).
    pub(crate) identity: veyndo_identity::service::bootstrap::IdentityState,
    /// Held for the process lifetime: this replica's share of the cluster connection budget.
    pub(crate) _connection_budget: Arc<performance::cluster_lease::Lease>,
}
impl App {
    /// The model context intelligence operations borrow: state, providers and inference slots.
    pub(crate) fn models(&self) -> veyndo_intelligence::service::models::Models<'_> {
        self.models.models(&self.core)
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
