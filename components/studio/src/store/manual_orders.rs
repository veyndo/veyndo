//! SQL of merchant manual orders: the active customer a draft is for, its admin cart context and the selectable sales channels.
use serde_json::Value;
use veyndo_core::{Result, State};
pub use veyndo_customers::store::manual_orders::Customer;

/// An active customer by ID.
pub async fn active_customer(a: &State, t: &str, id: &str) -> Result<Option<Customer>> {
    veyndo_customers::store::manual_orders::active_customer(a, t, id).await
}

/// Replaces a cart's data document.
pub async fn set_cart_data(a: &State, t: &str, cart: &str, data: &Value) -> Result<()> {
    veyndo_checkout::store::manual_orders::set_cart_data(a, t, cart, data).await
}

/// Up to 100 active public storefront/API channels as `{id,name}`.
pub async fn channels(a: &State, t: &str) -> Result<Vec<Value>> {
    veyndo_marketing::store::manual_orders::channels(a, t).await
}
