//! The cart rows coupon selection changes: the token-locked read and the revision bump. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::{StoredCart, upgrade_cart};

/// The cart behind `token`, locked for this transaction; a missing cart is an error.
pub async fn lock(conn: &mut PgConnection, tenant: &str, token: &str) -> Result<StoredCart> {
    let row = sqlx::query!(
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        tenant,
        token
    )
    .fetch_one(conn)
    .await?;
    let mut data = row.data;
    upgrade_cart(&mut data);
    Ok(StoredCart {
        id: row.id,
        tenant: row.tenant,
        token: row.token,
        data: serde_json::from_value(data).map_err(|e| bad(e.to_string()))?,
        revision: row.revision,
        status: row.status,
    })
}

/// Stores the cart document and advances its revision by one.
pub async fn save_next_revision(
    conn: &mut PgConnection,
    data: &Value,
    id: &str,
    tenant: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,revision=revision+1 WHERE tenant=$3 AND id=$2",
        data,
        id,
        tenant
    )
    .execute(conn)
    .await?;
    Ok(())
}
