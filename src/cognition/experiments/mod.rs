//! Controlled experiments live in the intelligence component; this module keeps the capability entry.
use crate::*;
pub(crate) use veyndo_intelligence::store::experiments::auto_once;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::experiments::invoke(&a.core, h, name, v).await
}
