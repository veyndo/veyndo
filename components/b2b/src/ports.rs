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
