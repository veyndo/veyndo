//! What the pricing component needs from other components. The server implements these; tests use their own.
use crate::catalog::Product;
use crate::types::currency::Config;
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::{RequestContext, State};

pub use veyndo_core::PortFuture;

/// The shop settings as far as pricing reads them.
pub trait ShopSettings: Send + Sync {
    /// Currency configuration and settings revision of a shop as one sales channel sees it.
    fn currencies<'a>(&'a self, tenant: &'a str, channel: &'a str)
    -> PortFuture<'a, (Config, i64)>;
    /// The shop's own currency configuration and revision, with the settings row locked until the transaction ends.
    fn currencies_for_update<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, (Config, i64)>;
}

/// The cart of a storefront request.
pub trait Carts: Send + Sync {
    /// The currency the request's cart was priced in; an unknown cart is an error.
    fn currency<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, String>;
}

/// The product catalogue as far as price jobs read and write it.
pub trait Catalogue: Send + Sync {
    /// Marks the changes of this transaction in the history as made by `actor` for `reason`.
    fn attribute<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        actor: Option<&'a str>,
        reason: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Up to 100 products after `cursor` up to `ceiling` that existed at `snapshot`, in ID order and locked.
    fn lock_products<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        cursor: &'a str,
        ceiling: &'a str,
        snapshot: &'a str,
    ) -> PortFuture<'a, Vec<Product>>;
    /// Stores the fixed prices of one currency on a product and advances its revision.
    fn store_fixed_prices<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        product: &'a str,
        currency: &'a str,
        prices: Value,
    ) -> PortFuture<'a, ()>;
}

/// Documents of components above pricing that refer to currencies or bound a price job, read in the caller's transaction.
pub trait References: Send + Sync {
    /// The highest product ID of the shop, or an empty string.
    fn product_ceiling<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, String>;
    /// Whether a product carries fixed prices in, or is priced in, one of the currencies.
    fn products_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool>;
    /// Whether an open cart is priced in one of the currencies.
    fn carts_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool>;
    /// Whether a purchasing unit buys in one of the currencies.
    fn units_use<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        tenant: &'a str,
        codes: &'a [String],
    ) -> PortFuture<'a, bool>;
}

/// The state pricing handlers extract: the core state and the ports.
#[derive(Clone)]
pub struct Pricing {
    pub core: State,
    pub settings: Arc<dyn ShopSettings>,
    pub carts: Arc<dyn Carts>,
    pub catalogue: Arc<dyn Catalogue>,
    pub references: Arc<dyn References>,
}
