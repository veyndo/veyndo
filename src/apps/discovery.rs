//! App capability discovery lives in the apps component; this module keeps the server-side entry for MCP and capabilities.
use super::service_limits;
use crate::foundation::App;
use serde_json::Value;
pub(crate) use veyndo_app_platform::domain::discovery::{CAPABILITIES, schema};
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_app_platform::service::discovery::invoke(&service_limits::platform(a), h, name, v).await
}
