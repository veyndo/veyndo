//! Settings locks of merchant workspace writes: the settings document and its revision, locked for update or share-locked inside the caller's transaction.
use serde_json::Value;
use veyndo_core::Result;

/// The settings document and its revision, locked for update.
pub async fn lock_settings(c: &mut sqlx::PgConnection, t: &str) -> Result<(Value, i64)> {
    let row = sqlx::query!(
        "SELECT data,revision FROM commerce_settings WHERE tenant=$1 FOR UPDATE",
        t
    )
    .fetch_one(&mut *c)
    .await?;
    Ok((row.data, row.revision))
}

/// The settings document and its revision, share-locked.
pub async fn share_settings(c: &mut sqlx::PgConnection, t: &str) -> Result<(Value, i64)> {
    let row = sqlx::query!(
        "SELECT data, revision FROM commerce_settings WHERE tenant=$1 FOR SHARE",
        t
    )
    .fetch_one(&mut *c)
    .await?;
    Ok((row.data, row.revision))
}
