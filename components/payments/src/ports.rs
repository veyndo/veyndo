//! What the payment ledger needs from the server: adapter service configuration and the effects other components attach to a captured order.
use crate::domain::attempt::Attempt;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{PortFuture, Result};

pub trait Host: Send + Sync {
    /// The configured adapter service of a provider version in one environment.
    fn service(&self, provider: &str, version: &str, environment: &str) -> Result<Value>;
    /// Runs inside the ledger transaction before the updated order is stored.
    fn captured<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        order: &'a mut Value,
        state: &'a str,
    ) -> PortFuture<'a, ()>;
}

/// The provider side of the ledger: the adapter transport and the worker's configured scope.
pub trait Gateway: Send + Sync {
    /// One admitted call to the pinned adapter service of a provider.
    fn call<'a>(
        &'a self,
        tenant: &'a str,
        provider: &'a str,
        version: &'a str,
        environment: &'a str,
        path: &'a str,
        body: &'a Value,
    ) -> PortFuture<'a, Value>;
    /// Executes a payment operation at the provider that owns the attempt.
    fn dispatch<'a>(
        &'a self,
        attempt: &'a Attempt,
        operation: &'a str,
        key: &'a str,
        input: &'a Value,
    ) -> PortFuture<'a, Value>;
    /// Admits the provider's evidence and applies its ledger effects inside the worker transaction.
    fn persist<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        attempt: &'a Attempt,
        operation: &'a str,
        input: &'a Value,
        evidence: &'a Value,
    ) -> PortFuture<'a, ()>;
    /// Tenants with a configured built-in account and the configured adapter providers.
    fn scope(&self) -> (Vec<String>, Vec<String>);
    /// Whether a built-in provider's order evidence permits cancelling the attempt.
    fn cancellable(&self, attempt: &Attempt, evidence: &Value) -> Result<()>;
    /// The installed app package that declares an adapter provider; an inactive one is refused.
    fn package<'a>(&'a self, tenant: &'a str, id: &'a str) -> PortFuture<'a, Manifest>;
    /// The status entry of the built-in provider for this tenant: environment, configuration and API contract.
    fn builtin(&self, tenant: &str) -> Value;
}
