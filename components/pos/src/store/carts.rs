//! The cashier's cart rows: the token-locked read and the revisioned writes of discount, customer and new-sale setup. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State, bad};
use veyndo_shop::types::cart::{StoredCart, upgrade_cart};

/// The cart behind `token`, locked for this transaction; older stored shapes are upgraded on the way.
pub async fn lock(
    conn: &mut PgConnection,
    tenant: &str,
    token: &str,
) -> Result<Option<StoredCart>> {
    let Some(row) = sqlx::query!(
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        tenant,
        token
    )
    .fetch_optional(conn)
    .await?
    else {
        return Ok(None);
    };
    let mut data = row.data;
    upgrade_cart(&mut data);
    Ok(Some(StoredCart {
        id: row.id,
        tenant: row.tenant,
        token: row.token,
        data: serde_json::from_value(data).map_err(|e| bad(e.to_string()))?,
        revision: row.revision,
        status: row.status,
    }))
}

/// Stores the cart document at its new revision.
pub async fn save(
    conn: &mut PgConnection,
    data: &Value,
    revision: i64,
    tenant: &str,
    id: &str,
) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,revision=$2 WHERE tenant=$3 AND id=$4",
        data,
        revision,
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores the document of a cart that was just created, keeping its revision.
pub async fn set_data(a: &State, data: &Value, tenant: &str, id: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1 WHERE tenant=$2 AND id=$3",
        data,
        tenant,
        id
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// A cart the cashier put on hold.
pub struct HeldCart {
    pub token: String,
    pub revision: i64,
    pub items: Value,
}

/// The cashier's open, non-empty carts in this channel, newest first (at most 40).
pub async fn held(a: &State, tenant: &str, channel: &str, actor: &str) -> Result<Vec<HeldCart>> {
    Ok(sqlx::query_as!(
        HeldCart,
        r#"SELECT token,revision,data->'items' AS "items!" FROM carts WHERE tenant=$1 AND status='open' AND data->>'sales_channel'=$2 AND data->>'pos_actor'=$3 AND jsonb_array_length(data->'items')>0 ORDER BY created_at DESC LIMIT 40"#,
        tenant,
        channel,
        actor
    )
    .fetch_all(&a.db)
    .await?)
}
