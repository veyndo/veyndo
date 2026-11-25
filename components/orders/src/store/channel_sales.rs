//! The recent orders of one sales channel as a register lists them. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

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
