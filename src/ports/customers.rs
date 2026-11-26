//! The server's implementation of the customers purchases port: buyer carts in the checkout store and buyer orders in the orders store.
use crate::foundation::App;
use serde_json::Value;
use sqlx::PgConnection;
use std::sync::Arc;
use veyndo_core::PortFuture;
use veyndo_customers::ports::Purchases;
use veyndo_customers::store::customers::OrderTotals;
use veyndo_shop::types::cart::StoredCart;

struct Server(veyndo_core::State);

impl Purchases for Server {
    fn lock_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        t: &'a str,
        token: &'a str,
    ) -> PortFuture<'a, StoredCart> {
        Box::pin(veyndo_checkout::store::customer_carts::lock_by_token(
            conn, t, token,
        ))
    }
    fn bind_cart<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        id: &'a str,
        data: &'a Value,
        token: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_checkout::store::customer_carts::bind(
            conn, id, data, token,
        ))
    }
    fn detach_carts<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        t: &'a str,
        email: &'a str,
        salt: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(veyndo_checkout::store::customer_carts::detach(
            conn, t, email, salt,
        ))
    }
    fn order_totals<'a>(
        &'a self,
        t: &'a str,
        customer: Option<&'a str>,
    ) -> PortFuture<'a, OrderTotals> {
        Box::pin(veyndo_orders::store::customer_orders::account_totals(
            &self.0, t, customer,
        ))
    }
    fn latest_guest_order<'a>(
        &'a self,
        t: &'a str,
        email: &'a str,
    ) -> PortFuture<'a, Option<(Value, String)>> {
        Box::pin(veyndo_orders::store::customer_orders::latest_guest(
            &self.0, t, email,
        ))
    }
}

/// Registers the purchases port with the process state.
pub(super) fn register(a: &App) {
    veyndo_customers::ports::register_purchases(Arc::new(Server(a.core.clone())));
}
