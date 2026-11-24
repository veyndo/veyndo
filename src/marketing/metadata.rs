//! Rule facts of products, customers and orders are saved by the marketing component; this module registers the route.
use crate::{App, SecureRoutes};
use axum::Router;
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/automation/entities/{entity}/{id}",
        &[("PUT", "automation.manage")],
        axum::routing::put(veyndo_marketing::http::rules::save_facts),
    )
}
