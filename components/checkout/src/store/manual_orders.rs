//! The admin cart context of a merchant manual order.
use serde_json::Value;
use veyndo_core::{Result, State};

/// Replaces a cart's data document.
pub async fn set_cart_data(a: &State, t: &str, cart: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1 WHERE tenant=$2 AND id=$3",
        data,
        t,
        cart
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
