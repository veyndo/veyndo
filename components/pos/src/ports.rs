//! What assisted selling (the cashier gateway and merchant manual orders) needs from the server: the guarded cart port, cart quotes, currency selection, checkout context changes, the purchase, catalogue reads and receipt snapshots.
use serde_json::Value;
use veyndo_catalog::types::catalog_page::CatalogCriteria;
use veyndo_checkout::ports::Quotes;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_shop::ports::Carts;

pub trait Register: Send + Sync {
    /// The cart of the request after its guards, its priced view and revision-checked line replacement.
    fn carts(&self) -> &dyn Carts;
    /// The server side of a cart quote, for the checkout options of the register.
    fn quotes(&self) -> &dyn Quotes;
    /// One currency capability (`currency.list`, `currency.select`) on the request.
    fn currency<'a>(
        &'a self,
        h: &'a RequestContext,
        name: &'a str,
        v: &'a Value,
    ) -> PortFuture<'a, Value>;
    /// Changes the checkout selection of the request's cart under its revision.
    fn select<'a>(&'a self, h: &'a RequestContext, v: Value) -> PortFuture<'a, Value>;
    /// Places the order of the request's cart with the reviewed revision and total.
    fn checkout<'a>(&'a self, h: &'a RequestContext, key: &'a str) -> PortFuture<'a, Value>;
    /// One page of the channel's catalogue.
    fn catalog<'a>(
        &'a self,
        h: &'a RequestContext,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value>;
    /// One product with its family and prices.
    fn detail<'a>(
        &'a self,
        h: &'a RequestContext,
        id: String,
        criteria: CatalogCriteria,
    ) -> PortFuture<'a, Value>;
    /// The immutable snapshot of an issued receipt after the document, order and PII rights.
    fn receipt<'a>(
        &'a self,
        h: &'a RequestContext,
        order: &'a str,
        id: &'a str,
    ) -> PortFuture<'a, Value>;
}
