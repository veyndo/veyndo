//! Native modular company procurement; protocol-specific EDI adapters remain in their app packages.
use crate::*;
mod access;
mod approval;
mod catalog;
mod contract;
mod gateway;
mod settings;
pub(crate) use contract::{CAPABILITIES, permission, schema};
mod model;
mod onboarding;
mod orders;
mod organization;
pub(crate) use organization::save as restore_configuration;
mod purchase;
mod quick_order;
mod quote_contract;
mod requisition;
use access::{access, catalog_guard, require};
pub(crate) use access::{guard, guard_conn, identity, surface_context};
pub(crate) use catalog::scope as catalog_scope;
pub(crate) use gateway::invoke;
pub(crate) use model::Context;
pub(crate) use purchase::preview;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/b2b/{operation}",
            &[("POST", "read")],
            post(gateway::admin),
        )
        .route("/store-api/b2b/{operation}", post(gateway::customer))
        .route(
            "/store-api/b2b/units/{unit}/orders/{id}/receipts/{receipt}/pdf",
            get(orders::receipt),
        )
}
