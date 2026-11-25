//! Which sales channels a shop has, for components that bind records to a channel. Columns checked at compile time.
use sqlx::{Executor, Postgres};
use veyndo_core::Result;

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
