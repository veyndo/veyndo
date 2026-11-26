//! What the payment ledger needs from the server: adapter service configuration, the cart and experiment records of an order's payment, the buyer account of a funding choice, the payment history of an order and the shop's sales channels, and the effects other components attach to a captured order.
use crate::domain::attempt::Attempt;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{PortFuture, Result};

/// Records of components that build on the payment ledger, written in the ledger transaction.
pub trait LedgerRecords: Send + Sync {
    /// Mirrors the order document into the order snapshot of the cart it was placed from.
    fn mirror_cart_order<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        cart: &'a str,
        order: &'a Value,
    ) -> PortFuture<'a, ()>;
    /// Completes the open experiment exposure of the cart's session once and counts the purchase for its variant.
    fn reward_capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        cart: &'a str,
    ) -> PortFuture<'a, ()>;
}

/// The buyer account a checkout funding choice belongs to.
pub trait Buyers: Send + Sync {
    /// The active customer of a current session (by token digest) of the shop.
    fn session_customer<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        digest: &'a str,
    ) -> PortFuture<'a, Option<String>>;
}

pub trait Host: LedgerRecords + Send + Sync {
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

/// One change of an order's payment in the native entity history.
pub struct PaymentChange {
    pub id: i64,
    pub created_at: String,
    pub state: Option<String>,
    pub captured: Option<Value>,
    pub refunded: Option<Value>,
}

/// Records of the merchant's shop the ledger reads: the native entity history of an order's payment and the sales channels accounts bind to.
pub trait MerchantRecords: Send + Sync {
    /// The latest 100 finalized changes of the order's payment, newest first.
    fn payment_history<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        order: &'a str,
    ) -> PortFuture<'a, Vec<PaymentChange>>;
    /// The sales channel exists in the shop.
    fn channel_exists<'a>(&'a self, tenant: &'a str, channel: &'a str) -> PortFuture<'a, bool>;
    /// The ids of the shop's sales channels.
    fn channel_ids<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Vec<String>>;
}

/// The provider side of the ledger: the adapter transport and the worker's configured scope.
pub trait Gateway: MerchantRecords + Send + Sync {
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
