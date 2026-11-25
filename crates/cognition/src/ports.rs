//! What the inference component needs from the server without depending on the owners: agent key budgets
//! (identity, which itself depends on inference) and the operator model configuration (shop).
use crate::scoped_pool::ScopedPool;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::PortFuture;

/// An agent key's budget and what it has allocated so far.
pub struct KeyUsage {
    pub agent_budget: Option<Value>,
    pub agent_tokens_allocated: i64,
    pub agent_cost_allocated: i64,
}

/// The platform AI configuration and its sealed provider secrets.
pub struct PlatformAi {
    pub data: Value,
    pub secrets: Value,
}

/// Agent keys and the operator configuration, read and charged for model admission.
pub trait InferenceHost: Send + Sync {
    /// The approved budget of an unexpired agent key of the live shop; a key without a budget fails.
    fn approved_budget<'a>(
        &'a self,
        db: &'a ScopedPool,
        live: &'a str,
        key: Option<&'a str>,
    ) -> PortFuture<'a, Option<Value>>;
    /// Locks an unexpired user agent key whose creator is still an active member, in the admission transaction.
    fn lock_key<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        live: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Option<KeyUsage>>;
    /// Adds a reservation to a locked key's allocated tokens and cost, in the admission transaction.
    fn allocate<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        live: &'a str,
        id: &'a str,
        tokens: i64,
        cost: i64,
    ) -> PortFuture<'a, ()>;
    /// The single platform AI row; the caller runs it in system scope.
    fn platform_ai<'a>(&'a self, db: &'a ScopedPool) -> PortFuture<'a, PlatformAi>;
}

static HOST: std::sync::OnceLock<Box<dyn InferenceHost>> = std::sync::OnceLock::new();

/// Registers the server's host once at startup; a second registration is ignored.
pub fn install(host: Box<dyn InferenceHost>) {
    let _ = HOST.set(host);
}

/// The registered host; `None` in processes without durable model admission.
pub fn host() -> Option<&'static dyn InferenceHost> {
    HOST.get().map(Box::as_ref)
}
