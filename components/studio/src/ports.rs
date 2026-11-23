//! What reviewed work and proposal approval need from the server: the native operation registry, the app gateway's managed saves and the catalogue state.
use serde_json::Value;
use veyndo_app_platform::domain::planning::AppChange;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_intelligence::ports::AgentHost;

/// The server side of reviewed work; binding, approval, execution and application stay in this component.
pub trait WorkHost: AgentHost {
    /// The native configuration operations the actor may run as reviewed work, with their input schemas.
    fn work_operations(&self, h: &RequestContext) -> Vec<(&'static str, Value)>;
    /// Binds an app change to the installed app's current revision and managed save contract.
    fn bind_change<'a>(
        &'a self,
        t: &'a str,
        change: &'a mut AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()>;
    /// Applies a bound app change inside the caller's transaction.
    fn apply_change<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        change: &'a AppChange,
        h: &'a RequestContext,
    ) -> PortFuture<'a, ()>;
    /// The catalogue state with its ports.
    fn catalog(&self) -> veyndo_catalog::ports::Catalog;
}
