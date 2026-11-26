//! Sales channels behaviour reports and experiments refer to: whether a channel belongs to the shop and the channel names a report offers.
use serde_json::Value;
use sqlx::PgExecutor;
use veyndo_core::Result;

/// Whether the channel belongs to the shop.
pub async fn channel_exists(e: impl PgExecutor<'_>, t: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2) AS "exists!""#,
        t,
        id
    )
    .fetch_one(e)
    .await?)
}

/// A channel's ID and localized name document.
pub struct ChannelName {
    pub id: String,
    pub name: Value,
}

/// Up to 100 channels of the shop with their name documents, by ID.
pub async fn channel_names(e: impl PgExecutor<'_>, t: &str) -> Result<Vec<ChannelName>> {
    Ok(sqlx::query_as!(ChannelName, r#"SELECT id,data->'name' AS "name!" FROM sales_channels WHERE tenant=$1 ORDER BY id LIMIT 100"#,
        t).fetch_all(e).await?)
}
