//! Proposal application lives in the studio component; these entries lend it the server's ports.
use crate::App;
use crate::agent_host::Host;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn apply(a: &App, t: &str, id: &str, h: &RequestContext) -> Result<Value> {
    veyndo_studio::service::proposal_apply::apply(&Host(a), t, id, h).await
}
pub(crate) async fn apply_mode(
    a: &App,
    t: &str,
    id: &str,
    h: &RequestContext,
    autonomous: bool,
) -> Result<Value> {
    veyndo_studio::service::proposal_apply::apply_mode(&Host(a), t, id, h, autonomous).await
}
