//! Bounded SKU bulk entry resolves native products then uses the ordinary cart's permissions, quantities and prices.
use crate::{App, RequestContext, Result, Value};
pub(super) use veyndo_b2b::service::quick_order::resolve;
pub(super) async fn apply(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_b2b::service::quick_order::apply(&crate::cart_storage::Carts(a), a, h, v).await
}
