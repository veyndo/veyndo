//! Grounded model planning lives in the intelligence component; the server keeps the native proposal admission it calls through the agent port.
use crate::*;
/// One planning turn over the server's agent port.
pub(crate) async fn plan_with(
    a: &App,
    t: &str,
    instruction: &str,
    choice: Option<&Choice>,
    history: &str,
    locale: &str,
    authority: &RequestContext,
) -> Result<Value> {
    let turn = veyndo_intelligence::service::planner::Turn {
        instruction,
        choice,
        history,
        locale,
    };
    veyndo_intelligence::service::planner::plan_with(
        &crate::agent_host::Host(a),
        t,
        turn,
        authority,
    )
    .await
}
