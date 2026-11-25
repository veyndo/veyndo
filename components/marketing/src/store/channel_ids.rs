//! The sales channels of a shop as components below marketing see them: their IDs under lock and their documents in ID order.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The IDs of all sales channels of the shop, locked until the transaction ends.
pub async fn lock_ids(tx: &mut PgConnection, tenant: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT id FROM sales_channels WHERE tenant=$1 ORDER BY id FOR UPDATE"#,
        tenant
    )
    .fetch_all(tx)
    .await?)
}

/// Up to `limit` sales channels with their documents, in ID order.
pub async fn first(a: &State, tenant: &str, limit: i64) -> Result<Vec<(String, Value)>> {
    Ok(sqlx::query!(
        "SELECT id,data FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT $2",
        tenant,
        limit
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.data))
    .collect())
}
