//! What the catalog component needs from other components. The server implements these; tests use their own.
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::{PortFuture, RequestContext, State};

/// Who writes a review: the cart of the request and, when signed in, its customer.
pub struct Shopper {
    pub tenant: String,
    pub cart: String,
    pub customer: Option<String>,
}

/// Carts and orders as far as reviews read them.
pub trait Shoppers: Send + Sync {
    fn current<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, Shopper>;
    /// Whether an order of this cart or this customer contains the product.
    fn bought<'a>(&'a self, shopper: &'a Shopper, product: &'a str) -> PortFuture<'a, bool>;
}

/// The shop around the catalogue: its environments and its change history.
pub trait Shop: Send + Sync {
    /// Whether the shop is a preview environment of a live shop.
    fn is_preview<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, bool>;
    /// Marks the changes of this transaction in the history as made by the request for `reason`.
    fn attribute<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        request: &'a RequestContext,
        reason: &'a str,
    ) -> PortFuture<'a, ()>;
}

/// The state catalog handlers extract when they need another component.
#[derive(Clone)]
pub struct Catalog {
    pub core: State,
    pub shoppers: Arc<dyn Shoppers>,
    pub shop: Arc<dyn Shop>,
}
