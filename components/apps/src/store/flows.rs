//! The active commerce flows an app upgrade must keep valid; read only.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The stored documents of every active flow of a shop.
pub async fn active(conn: &mut PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_flows WHERE tenant=$1 AND data->>'active'='true'",
        t
    )
    .fetch_all(conn)
    .await?)
}
