//! The order snapshot a cart keeps for its buyer, refreshed in the transaction that changed the order.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Replaces the cart's order snapshot with the order document.
pub async fn mirror(
    conn: &mut PgConnection,
    tenant: &str,
    cart: &str,
    order: &Value,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=jsonb_set(data,'{order}',$1) WHERE tenant=$2 AND id=$3",
        order,
        tenant,
        cart
    )
    .execute(conn)
    .await?;
    Ok(())
}
