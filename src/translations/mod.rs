//! Tenant-scoped, resumable AI translation drafts; applying is revision checked and emits native product events.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
mod apply;
mod routes;
pub(crate) mod selection;
mod worker;
pub(crate) use apply::apply;
pub(crate) use routes::{control, create, detail, list};
pub(crate) use worker::once;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/merchant/translations",
            &[("GET", "catalog.read"), ("POST", "catalog.write")],
            get(routes::list).post(routes::create),
        )
        .secure_route(
            "/api/merchant/translations/{id}",
            &[("GET", "catalog.read"), ("PUT", "catalog.write")],
            get(routes::detail).put(routes::control),
        )
        .secure_route(
            "/api/merchant/translations/{id}/apply",
            &[("POST", "catalog.write")],
            post(veyndo_catalog::http::translations::apply),
        )
}
