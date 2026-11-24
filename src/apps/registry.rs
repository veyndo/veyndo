//! Atomic installation and additive schema upgrades; immutable version digests preserve history.
use super::{Manifest, data, install_host, service_limits};
use crate::App;
use serde_json::Value;
use veyndo_app_platform::store::package_install;
pub(crate) use veyndo_app_platform::store::packages::package;
use veyndo_core::Result;
pub(crate) async fn install(a: &App, t: &str, m: Manifest) -> Result<Value> {
    package_install::install(&service_limits::platform(a), t, m).await
}
pub(crate) async fn install_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    package_install::install_tx(&data::BUNDLED, &install_host::COMMERCE, tx, t, m).await
}
