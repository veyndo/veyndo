//! Shared network-app admission lives in the apps component; this module builds its state from the server state.
use super::*;
pub(crate) use veyndo_app_platform::ports::Platform;
pub(crate) use veyndo_app_platform::service::admission::Admission;
pub(crate) use veyndo_app_platform::service::limits::ServiceLimits;
/// The app platform's view of the server state.
pub(crate) fn platform(a: &App) -> Platform {
    Platform {
        core: a.core.clone(),
        limits: a.app_limits.clone(),
    }
}
/// All app-network entry points share these ceilings, including signed delivery and pinned UI fetches.
pub(crate) async fn admit(a: &App, tenant: &str, app: &str) -> Result<Admission> {
    veyndo_app_platform::service::admission::admit(&platform(a), tenant, app).await
}
