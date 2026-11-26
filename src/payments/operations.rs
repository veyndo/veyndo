//! Durable idempotent payment commands, customer context binding and serial refund admission.
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn enqueue(
    a: &App,
    h: &RequestContext,
    id: &str,
    op: &str,
    key: &str,
    v: &Value,
) -> Result<Value> {
    veyndo_payments::store::commands::enqueue(a, h, id, op, key, v).await
}
pub(crate) async fn customer(a: &App, h: &RequestContext, id: &str) -> Result<()> {
    veyndo_payments::store::commands::customer(a, h, id).await
}
