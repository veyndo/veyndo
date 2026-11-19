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
/// Also invoked by ordinary cart adapters. A stolen POS token is not merchant or channel admission.
pub(crate) fn guard_cart(h: &RequestContext, c: &StoredCart) -> Result<()> {
    operations::guard_admin_cart(h, c)?;
    if c.data.pos_actor.is_some()
        && !h
            .pos
            .as_ref()
            .is_some_and(|p| c.data.pos_actor.as_deref() == Some(&p.actor))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS cart belongs to another cashier or requires POS admission".into(),
        ));
    }
    if c.data.sales_channel != marketing::channel_id(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Cart belongs to another sales channel".into(),
        ));
    }
    if c.data.pos_actor.is_some() && c.data.customer_id.is_some() {
        auth::permit(h, "customers.read")?;
        auth::permit(h, "customers.pii")?;
    }
    Ok(())
}
