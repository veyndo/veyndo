//! Bounded metadata-only app telemetry, sandbox transparency, explicit version rollback, typed port calls and explicitly approved cursor-based event replay.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
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
        .secure_route(
            "/api/apps/{id}/sandbox",
            &[("GET", "apps.manage")],
            get(veyndo_app_platform::http::sandbox::sandbox),
        )
        .secure_route(
            "/api/apps/{id}/rollback",
            &[("POST", "apps.manage")],
            post(veyndo_app_platform::http::sandbox::rollback),
        )
        .secure_route(
            "/api/apps/{id}/ports/{port}",
            &[("POST", "read")],
            post(veyndo_app_platform::http::sandbox::port),
        )
}
pub(super) use veyndo_app_platform::store::action_log::record;
