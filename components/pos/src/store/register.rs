//! What the register lists besides carts: the shop's POS channels, recent channel sales and barcode lookups. Columns checked at compile time.
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

/// One recent order of a channel as the register lists it.
pub struct Sale {
    pub id: String,
    pub number: String,
    pub money: Value,
    pub payment: String,
    pub pos: Option<Value>,
    pub created: String,
}

/// The channel's most recent orders (at most 30).
pub async fn sales(a: &State, tenant: &str, channel: &str) -> Result<Vec<Sale>> {
    Ok(sqlx::query_as!(
        Sale,
        r#"SELECT id,data->>'orderNumber' AS "number!",data->'money' AS "money!",data->'payment'->>'state' AS "payment!",data->'pos' AS pos,created_at::text AS "created!" FROM orders WHERE tenant=$1 AND data->>'salesChannelId'=$2 ORDER BY created_at DESC LIMIT 30"#,
        tenant,
        channel
    )
    .fetch_all(&a.db)
    .await?)
}

/// Active products whose id, number or EAN equals `barcode` (at most two, so ambiguity shows).
pub async fn barcode(a: &State, tenant: &str, barcode: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM products WHERE tenant=$1 AND active AND (id=$2 OR product_number=$2 OR extra->>'ean'=$2) ORDER BY id LIMIT 2",
        tenant,
        barcode
    )
    .fetch_all(&a.db)
    .await?)
}

/// The shop's main locale as stored in its settings.
pub async fn main_locale(conn: &mut PgConnection, tenant: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(
        r#"SELECT data->'mainLocale' AS "locale!: Value" FROM commerce_settings WHERE tenant=$1"#,
        tenant
    )
    .fetch_one(conn)
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
