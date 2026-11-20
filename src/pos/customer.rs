//! Cashier customer lookup and attachment are owned by the POS component; the server supplies the priced cart.
use super::*;
pub(super) async fn find(
    a: &App,
    h: &RequestContext,
    query: &str,
    card: Option<&str>,
) -> Result<Value> {
    veyndo_pos::service::customer::find(a, h, query, card).await
}
pub(super) async fn select(
    a: &App,
    h: &RequestContext,
    id: Option<&str>,
    revision: i64,
) -> Result<Value> {
    veyndo_pos::service::customer::select(&crate::cart_storage::Carts(a), a, h, id, revision).await
}
