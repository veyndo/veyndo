//! What the catalog component needs from other components. The server implements these; tests use their own.
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::localized::Languages;
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

/// The content languages of a shop, owned so they can cross a port.
pub struct ContentLanguages {
    pub main: String,
    pub enabled: Vec<String>,
}

impl ContentLanguages {
    pub fn as_ref(&self) -> Languages<'_> {
        Languages {
            main: &self.main,
            enabled: &self.enabled,
        }
    }
}

/// A storefront request resolved to its shop, language and the root of its channel's navigation.
pub struct Visit {
    pub tenant: String,
    pub locale: String,
    pub navigation_root: String,
}

/// The shop around the catalogue: its languages, its environments and its change history.
pub trait Shop: Send + Sync {
    fn languages<'a>(&'a self, tenant: &'a str) -> PortFuture<'a, ContentLanguages>;
    /// Resolves a storefront request; refuses a language the sales channel does not offer.
    fn visit<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, Visit>;
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
