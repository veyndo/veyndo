//! Storefront layout policy and session personalization in storage: the experience document, variant statistics, exposures, session behavior signals and channel call counters. Columns checked at compile time.
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};
use veyndo_core::Result;

/// The shop's experience document with its revision.
pub struct Experience {
    pub data: Value,
    pub revision: i64,
}

/// The shop's experience document.
pub async fn experience(e: impl PgExecutor<'_>, tenant: &str) -> Result<Experience> {
    Ok(sqlx::query_as!(
        Experience,
        "SELECT data,revision FROM experiences WHERE tenant=$1",
        tenant
    )
    .fetch_one(e)
    .await?)
}

/// The layout variant a session saw, with the probability it was chosen with.
pub struct Exposure {
    pub variant: String,
    pub propensity: f64,
}

/// The saved exposure of a session.
pub async fn exposure(
    e: impl PgExecutor<'_>,
    tenant: &str,
    session: &str,
) -> Result<Option<Exposure>> {
    Ok(sqlx::query_as!(
        Exposure,
        "SELECT variant,propensity FROM exposures WHERE tenant=$1 AND session=$2",
        tenant,
        session
    )
    .fetch_optional(e)
    .await?)
}

/// Observed views and purchases of one layout variant.
pub struct VariantStats {
    pub variant: String,
    pub views: i64,
    pub purchases: i64,
}

/// The statistics of every layout variant, by variant.
pub async fn variant_stats(e: impl PgExecutor<'_>, tenant: &str) -> Result<Vec<VariantStats>> {
    Ok(sqlx::query_as!(
        VariantStats,
        "SELECT variant,views,purchases FROM policy WHERE tenant=$1 ORDER BY variant",
        tenant
    )
    .fetch_all(e)
    .await?)
}

/// Records the session's first exposure and counts its view; an existing exposure stays.
pub async fn expose(
    tx: &mut PgConnection,
    tenant: &str,
    session: &str,
    variant: &str,
    propensity: f64,
) -> Result<()> {
    let n = sqlx::query!(
        "INSERT INTO exposures(tenant,session,variant,propensity) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
        tenant,
        session,
        variant,
        propensity
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 1 {
        sqlx::query!(
            "UPDATE policy SET views=views+1 WHERE tenant=$1 AND variant=$2",
            tenant,
            variant
        )
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

/// Serializes behavior signals of one session.
pub async fn lock_session(e: impl PgExecutor<'_>, key: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,18))", key)
        .execute(e)
        .await?;
    Ok(())
}

/// Behavior events of the session during the last hour.
pub async fn recent_events(e: impl PgExecutor<'_>, tenant: &str, session: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM session_signal_events WHERE tenant=$1 AND session=$2 AND created_at>now()-interval '1 hour'"#,
        tenant,
        session
    )
    .fetch_one(e)
    .await?)
}

/// Records a behavior event once and adds it to the session's product signal; false when the event was already recorded.
pub async fn signal(
    tx: &mut PgConnection,
    tenant: &str,
    event: &str,
    session: &str,
    product: &str,
    views: i32,
    cart_adds: i32,
) -> Result<bool> {
    let n = sqlx::query!(
        "INSERT INTO session_signal_events(tenant,id,session) VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
        tenant,
        event,
        session
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 1 {
        sqlx::query!("INSERT INTO session_signals(tenant,session,product_id,views,cart_adds) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant,session,product_id) DO UPDATE SET views=least(session_signals.views+EXCLUDED.views,1000),cart_adds=least(session_signals.cart_adds+EXCLUDED.cart_adds,1000),updated_at=now()",
            tenant, session, product, views, cart_adds).execute(&mut *tx).await?;
    }
    Ok(n == 1)
}

/// The weighted interest of a session in one category.
pub struct CategoryAffinity {
    pub category: String,
    pub score: i64,
}

/// The session's category interest over the last 30 days.
pub async fn affinities(
    e: impl PgExecutor<'_>,
    tenant: &str,
    session: &str,
) -> Result<Vec<CategoryAffinity>> {
    Ok(sqlx::query_as!(
        CategoryAffinity,
        r#"SELECT p.category,sum(s.views+s.cart_adds*3)::bigint AS "score!" FROM session_signals s JOIN products p ON p.tenant=s.tenant AND p.id=s.product_id WHERE s.tenant=$1 AND s.session=$2 AND s.updated_at>now()-interval '30 days' GROUP BY p.category"#,
        tenant,
        session
    )
    .fetch_all(e)
    .await?)
}

/// Whether the product exists in the shop.
pub async fn product_exists(e: impl PgExecutor<'_>, tenant: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND id=$2) AS "exists!""#,
        tenant,
        id
    )
    .fetch_one(e)
    .await?)
}

/// HTTP call counters of one sales channel.
pub struct ChannelCalls {
    pub channel: String,
    pub calls: i64,
    pub failures: i64,
}

/// The shop's HTTP call counters by channel.
pub async fn channel_calls(e: impl PgExecutor<'_>, tenant: &str) -> Result<Vec<ChannelCalls>> {
    Ok(sqlx::query_as!(
        ChannelCalls,
        "SELECT channel,calls,failures FROM channel_metrics WHERE tenant=$1 ORDER BY channel",
        tenant
    )
    .fetch_all(e)
    .await?)
}
