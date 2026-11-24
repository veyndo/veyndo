//! Once a minute: expired sign-in buckets, behaviour evidence and delivered outbox payloads are retired.
use crate::App;
use veyndo_core::Result;
pub(super) async fn once(a: &App) -> Result<()> {
    if !veyndo_core::outbox::retention::due() {
        return Ok(());
    }
    sqlx::query("DELETE FROM auth_attempt_buckets WHERE updated_at < now()-interval '1 day'")
        .execute(&a.db)
        .await?;
    crate::cognition::behavior::retire(a).await?;
    let config = crate::runtime_config::get();
    veyndo_core::outbox::retention::retire(
        a,
        config.outbox_retention_days as i32,
        config.outbox_metadata_days as i32,
    )
    .await
}
