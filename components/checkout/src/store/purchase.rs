//! Statements of the purchase transaction: the idempotency lock and replay, the order row, the completed cart and the experiment reward, all inside the caller's transaction. Columns checked at compile time.
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

/// The order an idempotency key already placed.
pub struct Placed {
    pub data: Value,
    pub fingerprint: String,
    pub cart_id: String,
}

/// The order of the shop placed under this idempotency key, if any.
pub async fn placed(tx: &mut PgConnection, tenant: &str, key: &str) -> Result<Option<Placed>> {
    Ok(sqlx::query_as!(
        Placed,
        r#"SELECT data,fingerprint AS "fingerprint!",cart_id FROM orders WHERE tenant=$1 AND idempotency_key=$2"#,
        tenant,
        key
    )
    .fetch_optional(tx)
    .await?)
}

/// The tenant's business policy module digest, share-locked for the purchase sandbox.
pub async fn policy_digest(tx: &mut PgConnection, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT digest FROM extensions WHERE tenant=$1 FOR SHARE",
        tenant
    )
    .fetch_one(tx)
    .await?)
}

/// A new order row.
pub struct NewOrder<'a> {
    pub id: &'a str,
    pub tenant: &'a str,
    pub cart: &'a str,
    pub key: &'a str,
    pub fingerprint: &'a str,
    pub data: &'a Value,
}

/// Inserts the order; the unique idempotency key rejects a concurrent duplicate.
pub async fn insert_order(tx: &mut PgConnection, o: &NewOrder<'_>) -> Result<()> {
    sqlx::query!("INSERT INTO orders(id,tenant,cart_id,idempotency_key,fingerprint,data) VALUES($1,$2,$3,$4,$5,$6)", o.id, o.tenant, o.cart, o.key, o.fingerprint, o.data)
        .execute(tx)
        .await?;
    Ok(())
}

/// Replaces the document of the order just inserted in this transaction.
pub async fn update_order(tx: &mut PgConnection, id: Option<&str>, data: &Value) -> Result<()> {
    sqlx::query!("UPDATE orders SET data=$1 WHERE id=$2", data, id)
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

/// Rewards the session's open experiment exposure once and counts the purchase for its variant.
pub async fn reward_exposure(tx: &mut PgConnection, tenant: &str, session: &str) -> Result<()> {
    if let Some(variant) = sqlx::query_scalar!("UPDATE exposures SET rewarded=true WHERE tenant=$1 AND session=$2 AND rewarded=false RETURNING variant", tenant, session)
        .fetch_optional(&mut *tx)
        .await?
    {
        sqlx::query!(
            "UPDATE policy SET purchases=purchases+1 WHERE tenant=$1 AND variant=$2",
            tenant,
            variant
        )
        .execute(tx)
        .await?;
    }
    Ok(())
}
