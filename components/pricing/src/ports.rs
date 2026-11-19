//! What the pricing component needs from other components. The server implements these; tests use their own.
use crate::types::currency::Config;
use std::{future::Future, pin::Pin, sync::Arc};
use veyndo_core::{RequestContext, Result, State};

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// The shop settings as far as pricing reads them.
pub trait ShopSettings: Send + Sync {
    /// Currency configuration and settings revision of a shop as one sales channel sees it.
    fn currencies<'a>(&'a self, tenant: &'a str, channel: &'a str)
    -> PortFuture<'a, (Config, i64)>;
    /// The shop's own currency configuration and revision, with the settings row locked until the transaction ends.
    fn currencies_for_update<'a>(
        &'a self,
        tx: &'a mut sqlx::PgConnection,
        tenant: &'a str,
    ) -> PortFuture<'a, (Config, i64)>;
}

/// The cart of a storefront request.
pub trait Carts: Send + Sync {
    /// The currency the request's cart was priced in; an unknown cart is an error.
    fn currency<'a>(&'a self, request: &'a RequestContext) -> PortFuture<'a, String>;
}

/// The state pricing handlers extract: the core state and the ports.
#[derive(Clone)]
pub struct Pricing {
    pub core: State,
    pub settings: Arc<dyn ShopSettings>,
    pub carts: Arc<dyn Carts>,
}
