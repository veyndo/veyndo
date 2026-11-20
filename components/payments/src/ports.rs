//! What the payment ledger needs from the server: adapter service configuration and the effects other components attach to a captured order.
use serde_json::Value;
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
