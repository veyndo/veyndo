//! Declared app settings reuse the digest-bound encrypted secret owner; secret values never have a browser read path.
use crate::{App, Router, SecureRoutes, get};
pub(super) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/apps/{id}/settings",
        &[("GET", "apps.manage"), ("PUT", "apps.manage")],
        get(veyndo_app_platform::http::settings::read)
            .put(veyndo_app_platform::http::settings::save),
    )
}
pub(super) use veyndo_app_platform::store::settings::resolve;
