//! Uniform tenant-scoped history from transactional database snapshots; reading lives in the studio component, restoration delegates to the owning domain validators.
use crate::{App, Router, SecureRoutes, get, post};
mod capability;
mod product;
mod restore;
mod routes;
pub(crate) use capability::{invoke, schema, visible};
use veyndo_studio::domain::history_access::{requires_pii, rights};
use veyndo_studio::http::history as transport;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/history/{entity}/{id}",
            &[("GET", "read")],
            get(transport::list),
        )
        .secure_route(
            "/api/history/{entity}/{id}/{version}",
            &[("GET", "read")],
            get(transport::detail),
        )
        .secure_route(
            "/api/history/{entity}/{id}/{version}/restore",
            &[("POST", "read")],
            post(routes::restore),
        )
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
}
