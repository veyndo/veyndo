//! Once a minute: expired sign-in buckets, behaviour evidence and delivered outbox payloads are retired.
use crate::foundation::App;
use veyndo_core::Result;
pub(super) async fn once(a: &App) -> Result<()> {
    if !veyndo_core::store::outbox::retention::due() {
        return Ok(());
    }
    veyndo_identity::store::attempts::retire(&a.core).await?;
    crate::cognition::behavior::retire(a).await?;
    let config = crate::runtime_config::get();
    veyndo_core::store::outbox::retention::retire(
        a,
        config.outbox_retention_days as i32,
        config.outbox_metadata_days as i32,
    )
    .await
}
