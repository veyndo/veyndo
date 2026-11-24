//! What quotes and procurement lists need from the server: the compiled app lifecycle of a new quote, app configuration admission and the configured cart edit.
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_shop::domain::configuration_selection::Selection;
use veyndo_shop::types::cart::Item;

/// The app lifecycle of quotes; the app platform compiles it from the active packages.
pub trait QuoteLifecycle: Send + Sync {
    /// The lifecycle snapshot a new quote carries, or null when no app declares one; held until the quote commits.
    fn capture<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, Value>;
}

/// What applying and saving a procurement list needs from the server: app configuration admission and the configured cart edit.
pub trait ListCarts: Send + Sync {
    /// Checks that a saved selection still resolves against the shop's current packages.
    fn check<'a>(&'a self, tenant: &'a str, selection: &'a Selection) -> PortFuture<'a, ()>;
    /// Replaces the request cart's lines and configurations at the expected revision and returns the priced cart.
    fn apply<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: i64,
        selections: &'a [Selection],
    ) -> PortFuture<'a, Value>;
}

/// What the company command gateway needs from components that depend on B2B: carts, lists, the quote lifecycle, customer sessions, order projection and state labels, the cashier guard, scoped settings, the settings writer and app extension fields.
pub trait Procurement: Send + Sync {
    /// The guarded request cart, its priced view and line replacement.
    fn carts(&self) -> &dyn veyndo_shop::ports::Carts;
    /// App configuration admission and the configured cart edit of procurement lists.
    fn lists(&self) -> &dyn ListCarts;
    /// The compiled app lifecycle of a new quote.
    fn lifecycle(&self) -> &dyn QuoteLifecycle;
    /// Refuses a request without a current customer session.
    fn customer<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, ()>;
    /// The customer-safe projection of one stored order.
    fn order(&self, raw: &Value, created: &str) -> Value;
    /// The configured order state labels of the shop, by state ID.
    fn labels<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, Value>;
    /// Refuses a cart that belongs to another cashier or a POS register.
    fn cashier(
        &self,
        h: &RequestContext,
        cart: &veyndo_shop::types::cart::StoredCart,
    ) -> veyndo_core::Result<()>;
    /// The settings of a sales channel under the basis row lock, with native rule conditions checked.
    fn locked<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        channel: &'a str,
    ) -> PortFuture<'a, (veyndo_shop::Settings, i64)>;
    /// The revisioned settings writer with its audit and outbox.
    fn save<'a>(&'a self, h: &'a RequestContext, v: Value) -> PortFuture<'a, Value>;
    /// Attaches the app extension fields of companies to the rows.
    fn companies<'a>(&'a self, h: &'a RequestContext, rows: &'a mut [Value]) -> PortFuture<'a, ()>;
}
