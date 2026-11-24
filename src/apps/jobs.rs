//! Durable long actions piggyback on the existing outbox. Apps claim a fenced lease; unknown side effects are never retried automatically.
use crate::{App, Router, SecureRoutes, get};
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/jobs",
            &[("GET", "apps.manage")],
            get(veyndo_app_platform::http::jobs::list),
        )
        .secure_route(
            "/api/apps/{id}/jobs/{job}",
            &[("GET", "read"), ("POST", "apps.manage")],
            get(veyndo_app_platform::http::jobs::detail)
                .post(veyndo_app_platform::http::jobs::control),
        )
}
