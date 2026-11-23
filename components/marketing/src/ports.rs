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

/// What saving marketing configuration needs from the server: actor-private work templates, installed app actions and channel setup of other components.
pub trait ConfigurationHost: Send + Sync {
    /// The shared database and caches.
    fn core(&self) -> &veyndo_core::State;
    /// The work template of a flow references completed work the actor may still copy.
    fn validate_template<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a veyndo_core::RequestContext,
        flow: &'a Flow,
    ) -> PortFuture<'a, ()>;
    /// Every app step of a flow names an installed, active action the actor may publish; `lock` fences concurrent installs.
    fn validate_apps<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        actor: &'a veyndo_core::RequestContext,
        flow: &'a Flow,
        lock: bool,
    ) -> PortFuture<'a, ()>;
    /// A newly saved channel of another kind (POS) prepares its own records in the same transaction.
    fn configure_channel<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        id: &'a str,
        data: &'a Value,
        created: bool,
    ) -> PortFuture<'a, ()>;
}
