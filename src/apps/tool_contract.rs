//! The app tool catalogue lives in the apps component; this module keeps the short path for MCP and the assistant.
use crate::App;
use serde_json::Value;
pub(crate) use veyndo_app_platform::store::tool_catalogue::app_tools_for;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn app_tools(a: &App, h: &RequestContext) -> Result<Vec<Value>> {
    app_tools_for(a, h, false).await
}
