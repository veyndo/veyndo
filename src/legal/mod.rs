//! Legal and privacy operations live in the intelligence component (consent) and the shop component (configuration, requests); the server registers their routes.
use crate::{App, SecureRoutes};
use axum::Router;
use axum::routing::get;
use axum::routing::post;
mod buyer_consent;
mod model;
pub(crate) use buyer_consent::missing_contact;
use veyndo_intelligence::http::legal;
pub(crate) use veyndo_intelligence::http::legal::{invoke, permission, schema};
use veyndo_intelligence::service::consent;
pub(crate) use veyndo_intelligence::service::consent::{
    continue_consent, read as consent_receipt, write_locked as consent_locked,
};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .route("/store-api/legal", get(consent::policy))
        .route(
            "/store-api/privacy/consent",
            get(consent::read).put(consent::write),
        )
        .route(
            "/store-api/legal/acceptance",
            axum::routing::put(legal::accept),
        )
        .route("/store-api/legal/requests", post(legal::create))
        .route("/store-api/legal/requests/{id}", get(legal::receipt))
        .secure_route(
            "/api/merchant/legal/requests",
            &[("GET", "customers.read")],
            get(legal::list),
        )
        .secure_route(
            "/api/merchant/legal/requests/{id}",
            &[("PUT", "customers.write")],
            axum::routing::put(legal::update),
        )
}

pub(crate) use model::version as policy_version;
