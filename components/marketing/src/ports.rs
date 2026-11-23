//! What the flow runner needs from the server: the effect of one pipeline action node.
use crate::types::flow::Flow;
use serde_json::Value;
use veyndo_core::PortFuture;

/// One action of a flow step: shop operations, app actions and payment instructions run in the server.
pub trait FlowEffects: Send + Sync {
    /// Runs the action once under its idempotency key and returns its recorded result.
    fn execute<'a>(
        &'a self,
        tenant: &'a str,
        flow: &'a Flow,
        key: &'a str,
        action: &'a str,
        config: &'a Value,
        definition: &'a Value,
    ) -> PortFuture<'a, Value>;
}
