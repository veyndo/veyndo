//! The order rows the purchase transaction places and replays, written on the purchase connection. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

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

/// The order a checkout session placed for this cart on this channel.
pub async fn session_order(
    tx: &mut PgConnection,
    tenant: &str,
    key: &str,
    cart: &str,
    channel: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!("SELECT data FROM orders WHERE tenant=$1 AND idempotency_key=$2 AND cart_id=$3 AND data->>'salesChannelId'=$4", tenant, key, cart, channel)
        .fetch_optional(tx)
        .await?)
}
