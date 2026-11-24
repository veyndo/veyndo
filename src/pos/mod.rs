//! Authenticated POS adapters reuse the native catalog/cart/checkout/receipt and durable event owners.
use crate::*;
mod config;
pub(crate) use veyndo_pos::service::discount::apply as adjust_quote;
mod gateway;
mod register_host;
mod schema;
mod settlement;
pub(crate) use config::configure;
pub(crate) use gateway::invoke;
pub(crate) use schema::CAPABILITIES;
pub(crate) use schema::schema;
pub(crate) use settlement::{check_locked, payment_captured, settle, validate_settlement};
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
