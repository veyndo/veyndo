//! Scoped app access to the existing asset store; the catalog component owns the listing and binary reads.
use crate::{App, RequestContext, Result, Value};
pub(crate) async fn callback(
    a: &App,
    h: &RequestContext,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    veyndo_catalog::service::asset_access::app_files(a, h, operation, v).await
}
