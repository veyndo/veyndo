//! Scoped checkout configuration: basis row lock orders all override writes and authoritative checkout reads.
use super::Settings;
use crate::{App, marketing, performance};
use std::sync::Arc;
use veyndo_core::Result;
pub(crate) async fn scoped_config(a: &App, t: &str, channel: &str) -> Result<(Arc<Settings>, i64)> {
    performance::settings(a, t, channel).await
}
pub(crate) async fn scoped_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    channel: &str,
) -> Result<(Settings, i64)> {
    veyndo_shop::store::settings::scoped_locked(tx, t, channel, &marketing::validate_condition)
        .await
}
