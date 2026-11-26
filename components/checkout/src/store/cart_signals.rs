//! Cart rows behaviour, consent and legal services lock or read: the cart lock that serializes consent changes and observation erasure, the cart state of an acknowledgement and experiment exposure removal.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// The stored cart document and status.
pub struct CartState {
    pub data: Value,
    pub status: String,
}

/// Locks the cart row so consent changes of one cart serialize.
pub async fn lock_cart(e: impl PgExecutor<'_>, tenant: &str, cart: &str) -> Result<()> {
    sqlx::query_scalar!(
        "SELECT revision FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        cart
    )
    .fetch_one(e)
    .await?;
    Ok(())
}

/// Deletes the experiment exposure of a session.
pub async fn forget_exposure(e: impl PgExecutor<'_>, tenant: &str, session: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM exposures WHERE tenant=$1 AND session=$2",
        tenant,
        session
    )
    .execute(e)
    .await?;
    Ok(())
}

/// The stored cart, row-locked.
pub async fn cart_locked(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<CartState> {
    Ok(sqlx::query_as!(
        CartState,
        "SELECT data,status FROM carts WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(e)
    .await?)
}
