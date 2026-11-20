//! Authenticated POS adapters reuse the native catalog/cart/checkout/receipt and durable event owners.
use crate::*;
mod config;
mod customer;
pub(crate) use customer::check as check_customer;
mod discount;
pub(crate) use discount::apply as adjust_quote;
mod gateway;
mod input;
mod schema;
mod settlement;
mod workspace_gateway;
pub(crate) use config::configure;
pub(crate) use gateway::invoke;
pub(crate) use schema::CAPABILITIES;
pub(crate) use schema::schema;
pub(crate) use settlement::{check_locked, payment_captured, settle, validate_settlement};
pub(crate) use veyndo_core::request_context::PosAdmission as Admission;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/pos/channels",
            &[("GET", "pos.use")],
            get(gateway::channels),
        )
        .secure_route(
            "/api/pos/{channel}/{operation}",
            &[("POST", "pos.use")],
            post(gateway::route),
        )
}
pub(crate) use veyndo_pos::service::guard::cart as guard_cart;
