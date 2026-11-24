//! Scoped app access to the existing asset store; the catalog component owns the listing and binary reads.
use crate::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn callback(
    a: &App,
    h: &RequestContext,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    veyndo_catalog::service::asset_access::app_files(a, h, operation, v).await
}
