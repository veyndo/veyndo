//! Bounded metadata-only app telemetry, storage usage and explicitly approved cursor-based event replay.
use super::*;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/activity",
            &[("GET", "apps.manage")],
            get(veyndo_app_platform::http::activity::activity),
        )
        .secure_route(
            "/api/apps/{id}/events/replay",
            &[("POST", "apps.manage")],
            post(veyndo_app_platform::http::activity::replay),
        )
}
pub(super) use veyndo_app_platform::store::action_log::record;
