//! What the shop needs from components that depend on it: the guarded cart of a buyer, the cross-component settings guards and the customers, orders, apps and rules its operator and settings views read.
use crate::types::cart::StoredCart;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_pricing::domain::cart::Item;

pub trait Carts: Send + Sync {
    /// The cart of the request after the POS, company and sales channel guards.
    fn load<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, StoredCart>;
    /// The priced cart as the storefront receives it.
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value>;
    /// Admits a cart row the caller locked itself: the cashier, sales channel and company guards on that connection.
    fn admit<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        h: &'a RequestContext,
        cart: &'a StoredCart,
    ) -> PortFuture<'a, ()>;
    /// Replaces the lines of the request's cart when the expected revision still holds.
    fn replace<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: Option<i64>,
    ) -> PortFuture<'a, StoredCart>;
}

/// What settings writes need from components that depend on the shop: rule conditions, payment capture configuration, the marketing configuration lock, app lifecycle locales and the documents that refer to a removed currency or method.
pub trait SettingsGuards:
    veyndo_pricing::ports::References + veyndo_shipping::ports::MethodReferences
{
    /// Checks one rule condition of the native rule engine.
    fn condition(&self, v: &Value) -> veyndo_core::Result<()>;
    /// Takes the marketing configuration lock that orders settings with rules, flows and promotions.
    fn lock<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Checks the payment capture configuration of resolved settings.
    fn capture<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        settings: &'a crate::Settings,
    ) -> PortFuture<'a, ()>;
    /// Refuses a main language that an active app lifecycle does not support.
    fn locale<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        t: &'a str,
        locale: &'a str,
    ) -> PortFuture<'a, ()>;
}

/// The state settings handlers run with: the shared state and the server's guards.
#[derive(Clone)]
pub struct SettingsHost {
    /// Database pool and HTTP client.
    pub core: veyndo_core::State,
    /// The cross-component checks of a settings write.
    pub guards: std::sync::Arc<dyn SettingsGuards>,
}

/// What one shop holds in the components above the shop, for the operator shop list.
#[derive(Default, Clone, Copy)]
pub struct Activity {
    pub customers: i64,
    pub orders: i64,
    pub apps: i64,
    pub sources: i64,
}

/// What the operator views, the settings view and the settings guards read from components that depend on the shop: customers, orders, apps, rules, flows, promotions and carts.
pub trait Dependants: Send + Sync {
    /// Customers of all shops outside `excluded`, on the caller's connection.
    fn customers_outside<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        excluded: &'a [String],
    ) -> PortFuture<'a, i64>;
    /// Orders of the last `days` days of all shops outside `excluded`, on the caller's connection.
    fn orders_outside<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        days: i64,
        excluded: &'a [String],
    ) -> PortFuture<'a, i64>;
    /// The amounts per currency of all shops outside `excluded`, or of `tenant`, for a window of `days`.
    fn amounts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        days: i64,
        tenant: Option<&'a str>,
        excluded: &'a [String],
    ) -> PortFuture<'a, Vec<crate::store::operator_metrics::Amounts>>;
    /// Customers, orders of the last `days` days, active apps and evidence sources per shop among `tenants`.
    fn activity<'a>(
        &'a self,
        tenants: &'a [String],
        days: i64,
    ) -> PortFuture<'a, std::collections::HashMap<String, Activity>>;
    /// The orders of a shop per day for the last `days` days, oldest first.
    fn timeline<'a>(
        &'a self,
        tenant: &'a str,
        days: i64,
    ) -> PortFuture<'a, Vec<crate::store::operator_metrics::Day>>;
    /// The newest thirty order documents without their cart token.
    fn recent_orders<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Vec<Value>>;
    /// The validated business identity of a shop's receipts.
    fn receipt_identity<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Option<Value>>;
    /// Locks a cart of the shop until the transaction ends; an unknown cart is an error.
    fn lock_cart<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Whether a customer of the shop belongs to a group outside `groups`.
    fn customers_outside_groups<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        groups: &'a [String],
    ) -> PortFuture<'a, bool>;
    /// The rule conditions, flow definitions and promotion definitions of the shop.
    fn marketing_documents<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Vec<Value>>;
}

static DEPENDANTS: std::sync::OnceLock<std::sync::Arc<dyn Dependants>> = std::sync::OnceLock::new();

/// Registers the server's dependants at startup; the first registration wins.
pub fn register_dependants(dependants: std::sync::Arc<dyn Dependants>) {
    let _ = DEPENDANTS.set(dependants);
}

/// The registered dependants; before registration every call fails.
pub fn dependants() -> veyndo_core::Result<&'static dyn Dependants> {
    DEPENDANTS.get().map(|d| &**d).ok_or_else(|| {
        veyndo_core::Error(
            veyndo_core::StatusCode::SERVICE_UNAVAILABLE,
            "Customers, orders, apps and rules are unavailable".into(),
        )
    })
}
