//! Accepted offer prices feed the existing currency/tax calculator; checkout consumes the offer atomically.
use crate::{App, RequestContext, Result, Value};
pub(super) async fn select(a: &App, h: &RequestContext, v: &Value, accept: bool) -> Result<Value> {
    veyndo_b2b::service::quote_contract::select(&crate::cart_storage::Carts(a), a, h, v, accept)
        .await
}
