//! The sales channel facts a checkout admits a purchase against, read on the checkout connection. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::Result;

/// The channel document, share-locked until the checkout transaction ends.
pub async fn share_document(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        tenant,
        channel
    )
    .fetch_optional(conn)
    .await?)
}

/// The channel's product selection (`productIds`) as stored; a missing channel is a database error.
pub async fn product_selection(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<Value> {
    Ok(sqlx::query_scalar!(r#"SELECT data->'productIds' AS "product_ids!" FROM sales_channels WHERE tenant=$1 AND id=$2"#, tenant, channel)
        .fetch_one(conn)
        .await?)
}

/// Whether the channel is active and public, share-locked; `None` when the shop has no such channel.
pub async fn share_open(
    conn: &mut PgConnection,
    tenant: &str,
    channel: &str,
) -> Result<Option<bool>> {
    Ok(sqlx::query_scalar!(r#"SELECT (data->>'active')::boolean AND coalesce(data->>'visibility','public')='public' AS "open!" FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE"#, tenant, channel)
        .fetch_optional(conn)
        .await?)
}
