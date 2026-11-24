//! API, MCP and flow source extraction lives in the intelligence component; this entry lends it the server's model context.
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn extract(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    veyndo_intelligence::service::extraction::extract(a.models(), h, v).await
}
