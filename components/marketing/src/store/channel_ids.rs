//! The sales channels of a shop as other components see them: existence, IDs (also under lock) and their documents in ID order. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
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

/// The sales channel exists in the shop.
pub async fn exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    channel: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2) AS "owned!""#,
        tenant,
        channel
    )
    .fetch_one(db)
    .await?)
}

/// The ids of the shop's sales channels.
pub async fn ids<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM sales_channels WHERE tenant=$1 ORDER BY id",
        tenant
    )
    .fetch_all(db)
    .await?)
}
