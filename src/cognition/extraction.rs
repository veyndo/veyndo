//! API, MCP and flow source extraction lives in the intelligence component; this entry lends it the server's model context.
use crate::{App, RequestContext, Result, Value};
pub(crate) async fn extract(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::extraction::extract(a.models(), h, v).await
}
