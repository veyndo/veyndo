//! The pipeline runner is owned by the marketing component; the server supplies the action effects.
use super::flow_jobs::Claim;
use super::flows;
use crate::App;
use serde_json::Value;
use veyndo_core::Result;
pub(super) use veyndo_marketing::service::pipeline_runtime::Outcome;

/// The server side of a flow action node.
struct Effects<'a>(&'a App);
impl veyndo_marketing::ports::FlowEffects for Effects<'_> {
    fn execute<'a>(
        &'a self,
        tenant: &'a str,
        flow: &'a flows::Flow,
        key: &'a str,
        action: &'a str,
        config: &'a Value,
        definition: &'a Value,
    ) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(super::flow_actions::execute(
            self.0, tenant, flow, key, action, config, definition,
        ))
    }
}
pub(super) async fn run(a: &App, job: &Claim, f: &flows::Flow) -> Result<Outcome> {
    veyndo_marketing::service::pipeline_runtime::run(a, &Effects(a), job, f).await
}
