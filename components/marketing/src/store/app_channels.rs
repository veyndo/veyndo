//! Sales channel and flow reads app records, surfaces, frontends and upgrades check: existence, the share-locked channel document and active flows.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// Whether the shop has the sales channel.
pub async fn exists<'e>(
    db: impl Executor<'e, Database = Postgres>,
    t: &str,
    id: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(db)
    .await?)
}

/// The sales channel document, share-locked for the rest of the transaction.
pub async fn lock_document(conn: &mut PgConnection, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        t,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// The stored documents of every active flow of a shop.
pub async fn active_flows(conn: &mut PgConnection, t: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_flows WHERE tenant=$1 AND data->>'active'='true'",
        t
    )
    .fetch_all(conn)
    .await?)
}
