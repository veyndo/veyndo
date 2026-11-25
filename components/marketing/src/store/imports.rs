//! Native promotion and channel state that reviewed migration imports replay against and compare with.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A promotion's data document.
pub async fn promotion(a: &State, t: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM commerce_promotions WHERE tenant=$1 AND id=$2",
        t,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Share-locks a sales channel; `false` when it does not exist.
pub async fn share_channel(c: &mut sqlx::PgConnection, t: &str, channel: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM sales_channels WHERE tenant=$1 AND id=$2 FOR SHARE",
        t,
        channel
    )
    .fetch_optional(&mut *c)
    .await?
    .is_some())
}
