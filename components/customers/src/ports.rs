//! What signing a buyer in needs from components that depend on customers: the cashier guard of the cart and its priced view.
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext, Result};
use veyndo_shop::types::cart::StoredCart;

pub trait SignIn: Send + Sync {
    /// Refuses a cart that belongs to a POS register or another cashier.
    fn cashier(&self, h: &RequestContext, cart: &StoredCart) -> Result<()>;
    /// The priced cart as the storefront receives it.
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value>;
}
