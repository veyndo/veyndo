//! What the customers component needs from components that depend on it: the cashier guard and priced view of a signed-in cart, and the carts and orders of a buyer.
use serde_json::Value;
use veyndo_core::{PortFuture, RequestContext, Result};
use veyndo_shop::types::cart::StoredCart;

pub trait SignIn: Send + Sync {
    /// Refuses a cart that belongs to a POS register or another cashier.
    fn cashier(&self, h: &RequestContext, cart: &StoredCart) -> Result<()>;
    /// The priced cart as the storefront receives it.
    fn view<'a>(&'a self, cart: &'a StoredCart) -> PortFuture<'a, Value>;
}

/// What a customer's order metrics, guest contacts and buyer sign-in need from the carts and orders above the customers component.
pub trait Purchases: Send + Sync {
    /// The cart of a context token under an update lock.
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        t: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, StoredCart>;
    /// Binds the cart document to the account and rotates its context token.
    fn bind_cart<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        id: &'a str,
        data: &'a Value,
        token: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Detaches the open carts of an email from its account and rotates their tokens with `salt`.
    fn detach_carts<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        t: &'a str,
        email: &'a str,
        salt: &'a str,
    ) -> PortFuture<'a, ()>;
    /// Count, total amount and latest date of the orders bound to the account ID.
    fn order_totals<'a>(
        &'a self,
        t: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, crate::store::customers::OrderTotals>;
    /// The newest guest order of an email: its snapshot and creation time.
    fn latest_guest_order<'a>(
        &'a self,
        t: &'a str,
        email: &'a str,
    ) -> PortFuture<'a, Option<(Value, String)>>;
}

static PURCHASES: std::sync::OnceLock<std::sync::Arc<dyn Purchases>> = std::sync::OnceLock::new();

/// Registers the server's carts and orders at startup; the first registration wins.
pub fn register_purchases(purchases: std::sync::Arc<dyn Purchases>) {
    let _ = PURCHASES.set(purchases);
}

/// The registered carts and orders; before registration every call fails.
pub fn purchases() -> Result<&'static dyn Purchases> {
    PURCHASES.get().map(|p| &**p).ok_or_else(|| {
        veyndo_core::Error(
            veyndo_core::StatusCode::SERVICE_UNAVAILABLE,
            "Carts and orders are unavailable".into(),
        )
    })
}
