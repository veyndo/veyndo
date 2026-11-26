//! Checkout's statements of the purchase transaction: the idempotency lock, the completed cart and the experiment exposure reward, all inside the caller's transaction; order rows belong to `veyndo_orders::store::placement`. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// Serializes purchases with the same tenant-scoped idempotency key until the transaction ends.
pub async fn lock_key(tx: &mut PgConnection, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,0))", key)
        .execute(tx)
        .await?;
    Ok(())
}

/// Completes the cart with its order snapshot.
pub async fn complete_cart(tx: &mut PgConnection, id: &str, data: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET status='completed',data=$1,revision=revision+1 WHERE id=$2",
        data,
        id
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Rewards the session's open experiment exposure once; returns the variant whose purchase the policy counts.
pub async fn reward_exposure(
    tx: &mut PgConnection,
    tenant: &str,
    session: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("UPDATE exposures SET rewarded=true WHERE tenant=$1 AND session=$2 AND rewarded=false RETURNING variant", tenant, session)
        .fetch_optional(tx)
        .await?)
}

/// Rewards the open experiment exposure of the session a cart belongs to; returns the variant whose purchase the policy counts.
pub async fn reward_cart_exposure(
    tx: &mut PgConnection,
    tenant: &str,
    cart: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("UPDATE exposures SET rewarded=true WHERE tenant=$1 AND session=(SELECT data->>'session' FROM carts WHERE tenant=$1 AND id=$2) AND rewarded=false RETURNING variant", tenant, cart)
        .fetch_optional(tx)
        .await?)
}
