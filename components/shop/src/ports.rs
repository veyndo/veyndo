//! What components that work on a buyer's cart need from the server: the guarded cart, its priced view and a revision-checked replacement of its lines.
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

/// What settings writes need from components that depend on the shop: rule conditions, payment capture configuration, the marketing configuration lock and app lifecycle locales.
pub trait SettingsGuards: Send + Sync {
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
