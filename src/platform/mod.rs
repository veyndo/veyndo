//! Global SaaS control plane: operator-only aggregate statistics and audited shop provisioning.
use crate::{App, Router, SecureRoutes, get, post};
mod auth;
mod bootstrap;
pub(crate) use bootstrap::bootstrap_operator;
mod ai;
mod infrastructure;
mod lifecycle;
mod metrics;
mod quotas;
mod resources;
mod shop_detail;
pub(crate) use lifecycle::admit;
mod provision;
pub(crate) use auth::authenticate;
use veyndo_shop::http::operator_session::{audit, session};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/platform/session",
            &[("GET", "platform")],
            get(session),
        )
        .secure_route(
            "/api/platform/overview",
            &[("GET", "platform")],
            get(metrics::overview),
        )
        .secure_route(
            "/api/platform/shops",
            &[("GET", "platform"), ("POST", "platform")],
            get(metrics::shops).post(provision::create),
        )
        .secure_route(
            "/api/platform/shops/{id}",
            &[("GET", "platform")],
            get(shop_detail::detail),
        )
        .secure_route(
            "/api/platform/shops/{id}/status",
            &[("POST", "platform")],
            post(lifecycle::change),
        )
        .secure_route(
            "/api/platform/shops/{id}/quotas",
            &[("GET", "platform"), ("PUT", "platform")],
            get(quotas::get).put(quotas::save),
        )
        .secure_route(
            "/api/platform/ai",
            &[("GET", "platform"), ("PUT", "platform")],
            get(ai::get).put(ai::save),
        )
        .secure_route(
            "/api/platform/infrastructure",
            &[("GET", "platform")],
            get(infrastructure::snapshot),
        )
        .secure_route("/api/platform/audit", &[("GET", "platform")], get(audit))
}
