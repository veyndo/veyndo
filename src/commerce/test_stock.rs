//! Entry point for scenarios; test stock is owned by the inventory component.
use crate::{App, Result, Value};
pub(crate) async fn fill(a: &App, t: &str, quantity: i32) -> Result<Value> {
    veyndo_inventory::service::test_stock::fill(&axum::extract::FromRef::from_ref(a), t, quantity)
        .await
}
