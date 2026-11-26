//! One fresh admission read per request: the hosted mount, the addressed shop, its live environment status and the sales channel. Columns checked at compile time.
use serde_json::Value;
use std::sync::Arc;
use veyndo_core::access::{AccessSnapshot, ChannelSnapshot, FrontendMount};
use veyndo_core::{Result, State};

/// The admission row of `access_snapshot.sql`.
struct Row {
    tenant: String,
    channel: String,
    exists: bool,
    parent: Option<String>,
    status: Option<String>,
    channel_data: Option<Value>,
    channel_revision: Option<i64>,
    origin: Option<String>,
    experience_alias: Option<String>,
}

/// Reads the snapshot under the request's MVCC view; a hosted alias decides the shop and channel, never headers.
pub async fn load(
    a: &State,
    alias: Option<&str>,
    tenant: &str,
    channel: &str,
) -> Result<Arc<AccessSnapshot>> {
    let r = sqlx::query_file_as!(Row, "src/store/access_snapshot.sql", alias, tenant, channel)
        .fetch_one(&a.db)
        .await?;
    Ok(Arc::new(AccessSnapshot {
        tenant: r.tenant,
        channel_id: r.channel,
        exists: r.exists,
        parent: r.parent,
        status: r.status,
        channel: r.channel_data.map(|data| ChannelSnapshot {
            data,
            revision: r.channel_revision.unwrap_or_default(),
        }),
        mount: r.origin.map(|origin| FrontendMount {
            origin,
            experience_alias: r.experience_alias.unwrap_or_default(),
        }),
    }))
}

/// The lifecycle status of the live shop behind a shop or its sandbox.
pub async fn live_status(a: &State, t: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("SELECT t.status FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=$1),$1)", t).fetch_optional(&a.db).await?)
}
