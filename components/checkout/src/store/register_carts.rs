//! The POS register's cart rows: the revisioned document writes of a cashier sale and the cashier's held carts. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

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
