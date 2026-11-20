//! Revision-bound purchasing approval inbox; approvers must be active peers and cannot approve themselves.
use crate::{App, RequestContext, Result, Value};
pub(super) use veyndo_b2b::service::approval::{decide, inbox};
pub(super) async fn request(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_b2b::service::approval::request(&crate::cart_storage::Carts(a), a, h, v).await
}
