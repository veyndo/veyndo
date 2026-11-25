//! Reads of the staging registry (`shop_environments`): the live parent of a stage and a stage's baseline.
use crate::{Error, Result, StatusCode};

/// The live shop a staging environment belongs to; `None` for a live shop.
pub async fn live_parent(conn: &mut sqlx::PgConnection, t: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT live_tenant FROM shop_environments WHERE tenant=$1")
            .bind(t)
            .fetch_optional(conn)
            .await?,
    )
}
/// The baseline of a shared stage that belongs to a live shop; personal previews and foreign stages are unavailable.
pub async fn owned_stage<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    live: &str,
    stage: &str,
) -> Result<serde_json::Value> {
    sqlx::query_scalar("SELECT baseline FROM shop_environments WHERE tenant=$1 AND live_tenant=$2 AND preview_owner IS NULL")
        .bind(stage)
        .bind(live)
        .fetch_optional(db)
        .await?
        .ok_or(Error(
            StatusCode::NOT_FOUND,
            "Environment unavailable".into(),
        ))
}
/// The live shop a staging environment belongs to, read on the pooled connection; `None` for a live shop.
pub async fn parent(a: &crate::State, t: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT live_tenant FROM shop_environments WHERE tenant=$1")
            .bind(t)
            .fetch_optional(&a.db)
            .await?,
    )
}
