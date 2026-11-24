//! The source-bound claim lifecycle lives in the intelligence component; these entries keep the server-side signatures.
use crate::{App, RequestContext, Result, Value};
use veyndo_intelligence::service::claims as lifecycle;
pub(crate) use veyndo_intelligence::store::evidence::{NODE_TYPES, claims};
pub(crate) async fn propose(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    lifecycle::propose(&a.core, h, v).await
}
pub(crate) async fn decide(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    lifecycle::decide(&a.core, h, v).await
}
