//! The POS sales channels of a shop and the channel fields a cashier sale reads. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// A POS sales channel with its document and revision.
pub struct PosChannelRow {
    pub id: String,
    pub data: Value,
    pub revision: i64,
}

/// The shop's POS channels by id (at most 100).
pub async fn channels(a: &State, tenant: &str) -> Result<Vec<PosChannelRow>> {
    Ok(sqlx::query_as!(
        PosChannelRow,
        "SELECT id,data,revision FROM sales_channels WHERE tenant=$1 AND data->>'kind'='pos' ORDER BY id LIMIT 100",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// The receipt message of a POS channel, `{}` when it has none.
pub async fn receipt_message(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        r#"SELECT coalesce(data->'pos'->'receiptMessage','{}'::jsonb) AS "message!" FROM sales_channels WHERE tenant=$1 AND id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}

/// The channel document, share-locked so it cannot change before the sale commits.
pub async fn channel_for_share(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}
