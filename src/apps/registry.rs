//! Atomic installation and additive schema upgrades; immutable version digests preserve history.
use super::{Manifest, data, install_host, service_limits};
use crate::foundation::App;
use serde_json::Value;
use veyndo_app_platform::store::package_install;
pub(crate) use veyndo_app_platform::store::packages::package;
use veyndo_core::Result;
/// Installs a package with its signed contract (agent effects, ports, rollbacks).
pub(crate) async fn install_package(
    a: &App,
    t: &str,
    m: Manifest,
    contract: Option<&veyndo_app_platform::domain::package_contract::Contract>,
) -> Result<Value> {
    package_install::install_package(&service_limits::platform(a), t, m, contract).await
}
pub(crate) async fn install_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    package_install::install_tx(&data::BUNDLED, &install_host::COMMERCE, tx, t, m).await
}
