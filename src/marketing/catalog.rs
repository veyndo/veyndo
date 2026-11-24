//! The automation catalogue is owned by the marketing component; the server registers its routes.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
pub(super) use veyndo_marketing::http::catalog::{catalog, import};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/automation/catalog",
            &[("GET", "automation.read")],
            get(catalog),
        )
        .secure_route(
            "/api/automation/import-condition",
            &[("POST", "automation.manage")],
            post(import),
        )
}
