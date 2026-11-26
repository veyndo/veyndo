//! Shared merchant payment operation bridge; the payments component owns the rights and durable jobs.
use super::host::{Gateway, PaymentHost};
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_payments::service::merchant_operations::invoke(
        (&PaymentHost, &Gateway(a)),
        a,
        h,
        name,
        v,
    )
    .await
}
