//! Reviewed work lives in the studio component; these entries lend it the server's ports.
use super::*;
use crate::agent_host::Host;
pub(crate) use veyndo_studio::domain::proposal::work_schema as schema;
pub(crate) fn catalogue(a: &App, h: &RequestContext) -> Vec<Value> {
    veyndo_studio::service::work_plan::catalogue(&Host(a), h)
}
pub(crate) async fn execute(
    a: &App,
    t: &str,
    id: &str,
    h: &RequestContext,
    request: &Value,
) -> Result<Value> {
    veyndo_studio::service::work_execution::execute(&Host(a), t, id, h, request).await
}
