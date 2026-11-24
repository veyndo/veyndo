//! Shared network-app admission lives in the apps component; this module builds its state from the server state.
use super::{data, gateway, install_host};
use crate::foundation::App;
pub(crate) use veyndo_app_platform::ports::Platform;
pub(crate) use veyndo_app_platform::service::admission::Admission;
pub(crate) use veyndo_app_platform::service::limits::ServiceLimits;
use veyndo_core::Result;
/// The app platform's view of the server state.
pub(crate) fn platform(a: &App) -> Platform {
    Platform {
        core: a.core.clone(),
        limits: a.app_limits.clone(),
        rules: &data::BUNDLED,
        commerce: &install_host::COMMERCE,
        actions: std::sync::Arc::new(gateway::ServerActions(a.clone())),
    }
}
impl axum::extract::FromRef<App> for Platform {
    fn from_ref(app: &App) -> Self {
        platform(app)
    }
}
/// All app-network entry points share these ceilings, including signed delivery and pinned UI fetches.
pub(crate) async fn admit(a: &App, tenant: &str, app: &str) -> Result<Admission> {
    veyndo_app_platform::service::admission::admit(&platform(a), tenant, app).await
}
