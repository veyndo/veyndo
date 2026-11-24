//! Controlled experiments live in the intelligence component; this module keeps the capability entry.
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_intelligence::store::experiments::auto_once;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::experiments::invoke(&a.core, h, name, v).await
}
