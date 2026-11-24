//! UI bundle loading lives in the apps component; this module registers its routes.
use crate::{App, SecureRoutes};
use axum::Router;
use axum::routing::post;
use veyndo_app_platform::http::ui_bundles::load;
pub(super) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/apps/{id}/surfaces/{surface}/bundle",
            &[("POST", "read")],
            post(load),
        )
        .route("/store-api/apps/{id}/surfaces/{surface}/bundle", post(load))
}
