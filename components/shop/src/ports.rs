//! What components that work on a buyer's cart need from the server: the guarded cart, its priced view and a revision-checked replacement of its lines.
use crate::types::cart::StoredCart;
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext};
use veyndo_pricing::domain::cart::Item;

pub trait Carts: Send + Sync {
    /// The cart of the request after the POS, company and sales channel guards.
    fn load<'a>(&'a self, h: &'a RequestContext) -> PortFuture<'a, StoredCart>;
    /// The priced cart as the storefront receives it.
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value>;
    /// Replaces the lines of the request's cart when the expected revision still holds.
    fn replace<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: Option<i64>,
    ) -> PortFuture<'a, StoredCart>;
}
