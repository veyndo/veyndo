//! Cart creation, loading and input validation.
use crate::*;

pub(crate) async fn load_cart(a: &App, h: &RequestContext) -> Result<StoredCart> {
    veyndo_checkout::store::cart_load::load(a, h).await
}
pub(crate) use veyndo_checkout::store::cart_create::new_cart_context;
pub(crate) use veyndo_shop::domain::cart_lines::validate as validate_items;

/// The server side of the cart port components use.
pub(crate) struct Carts<'s>(pub(crate) &'s App);
impl veyndo_shop::ports::Carts for Carts<'_> {
    fn load<'a>(&'a self, h: &'a RequestContext) -> veyndo_core::PortFuture<'a, StoredCart> {
        Box::pin(load_cart(self.0, h))
    }
    fn view<'a>(&'a self, cart: &'a StoredCart) -> veyndo_core::PortFuture<'a, Value> {
        Box::pin(cart_json(self.0, cart))
    }
    fn admit<'a>(
        &'a self,
        conn: &'a mut sqlx::PgConnection,
        h: &'a RequestContext,
        cart: &'a StoredCart,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            pos::guard_cart(h, cart)?;
            b2b::guard_conn(conn, h, cart).await?;
            Ok(())
        })
    }
    fn replace<'a>(
        &'a self,
        h: &'a RequestContext,
        items: Vec<Item>,
        expected: Option<i64>,
    ) -> veyndo_core::PortFuture<'a, StoredCart> {
        Box::pin(set_cart(self.0, h, items, expected, None))
    }
}
