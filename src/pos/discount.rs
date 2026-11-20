//! Cashier discounts are owned by the POS component; the server supplies the priced cart.
use super::*;
pub(crate) use veyndo_pos::service::discount::apply;
pub(super) async fn save(a: &App, h: &RequestContext, v: &Value, revision: i64) -> Result<Value> {
    veyndo_pos::service::discount::save(&crate::cart_storage::Carts(a), a, h, v, revision).await
}
