//! Atomic installation and additive schema upgrades; required hooks preserve compiler/trust, commerce, schema, default-record and schedule admission in the shared transaction.
use crate::domain::manifest_validation::validate;
use crate::domain::package_contract::{Contract, Rollback};
use crate::domain::{approval, field_values};
use crate::ports::{CommerceInstall, Platform, RecordRules};
use crate::service::configuration_runtime::validate_fee;
use crate::service::package_verification::verify_package;
use crate::store::{
    commerce_hooks, dependencies, managed_data, native_lifecycle, package_contracts, records,
    schedules, schema_upgrade, storage_usage,
};
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::installation::{HookFuture, InstallError, InstallHooks};
use veyndo_core::{Error, Result, bad, conflict, process_config};

/// The checks and side effects of one installation, run by the shared installer in its transaction.
pub struct Host<'a> {
    pub tenant: &'a str,
    pub manifest: &'a Manifest,
    pub rules: &'a dyn RecordRules,
    pub commerce: &'a dyn CommerceInstall,
    /// The data steps of an explicit rollback, taken from the contract of the version being replaced.
    pub rollback: Option<&'a Rollback>,
}
impl<'db> InstallHooks<'db> for Host<'_> {
    type Error = Error;
    fn prepare<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let (t, m) = (self.tenant, self.manifest);
            crate::store::locks::tenant_scope(tx, t).await?;
            let config = process_config::get();
            validate(m, &config.publishers)?;
            approval::installation(m, &config.services)?;
            verify_package(m).await?;
            self.commerce.lock(tx, t).await?;
            self.commerce.install(tx, t, m).await?;
            if let Some(c) = &m.configuration {
                let entity = m.entities.iter().find(|e| e.name == c.entity).unwrap();
                field_values::validate(entity, &c.default_fields)?;
                let fee = c.default_fields[&c.price_field]
                    .as_i64()
                    .ok_or(bad("Default price required"))?;
                validate_fee(c, fee).await?;
            }
            Ok(())
        })
    }
    fn admit<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            dependencies::check(
                tx,
                self.tenant,
                self.manifest,
                &process_config::get().publishers,
            )
            .await?;
            commerce_hooks::admission(tx, self.tenant, self.manifest).await?;
            native_lifecycle::admission(tx, self.tenant, self.manifest).await
        })
    }
    fn upgrade<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
        previous: &'a Value,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let old: Manifest = serde_json::from_value(previous.clone())
                .map_err(|_| bad("Invalid prior package"))?;
            if old.version != self.manifest.version {
                schema_upgrade::retire_snapshot(tx, self.tenant, &old.id, &self.manifest.version)
                    .await?;
            }
            if !older(&self.manifest.version, &old.version) {
                return schema_upgrade::upgrade(tx, self.tenant, &old, self.manifest).await;
            }
            // A downgrade only runs as an explicit rollback with the data steps the newer version declared.
            let Some(rollback) = self
                .rollback
                .filter(|r| r.to_version == self.manifest.version)
            else {
                return Err(conflict(
                    "Downgrade requires an explicit compatible migration",
                ));
            };
            let mut target = self.manifest.clone();
            target.schema_migrations = vec![veyndo_app_model::evolution::Migration {
                from_version: old.version.clone(),
                steps: rollback.steps.clone(),
            }];
            schema_upgrade::upgrade(tx, self.tenant, &old, &target).await
        })
    }
    fn attach<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            storage_usage::attach(tx, self.tenant, self.manifest).await?;
            records::install(tx, self.tenant, self.manifest).await
        })
    }
    fn finish<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
    ) -> HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            let (t, m) = (self.tenant, self.manifest);
            if let Some(c) = &m.configuration {
                let exists = records::get(tx, t, &m.id, &c.entity, &c.record, false)
                    .await?
                    .is_some();
                if !exists {
                    let entity = m.entities.iter().find(|e| e.name == c.entity).unwrap();
                    managed_data::save_tx(
                        self.rules,
                        tx,
                        t,
                        m,
                        entity,
                        &json!({"id":c.record,"fields":c.default_fields}),
                    )
                    .await?;
                }
            }
            schedules::sync(tx, t, m).await
        })
    }
}
/// Whether `new` is an earlier version than `old`: semantic versions when both are, numeric parts otherwise.
fn older(new: &str, old: &str) -> bool {
    match (semver::Version::parse(new), semver::Version::parse(old)) {
        (Ok(new), Ok(old)) => new < old,
        _ => {
            let parts = |s: &str| {
                s.split('.')
                    .map(|v| v.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>()
            };
            parts(new) < parts(old)
        }
    }
}
/// The response for a refused or failed installation.
pub fn error(error: InstallError<Error>) -> Error {
    match error {
        InstallError::Database(error) => error.into(),
        InstallError::Invalid(message) => bad(message),
        InstallError::Conflict(message) => conflict(message),
        InstallError::Hook(error) => error,
    }
}
/// Installs or upgrades a package with its signed contract in one privileged transaction.
pub async fn install_package(
    a: &Platform,
    t: &str,
    m: Manifest,
    contract: Option<&Contract>,
) -> Result<Value> {
    let mut tx = a.db.begin_privileged().await?;
    let result = install_inner(a.rules, a.commerce, &mut tx, t, m, true, contract, None).await?;
    tx.commit().await?;
    Ok(result)
}
/// Explicitly returns an installed package to an earlier published version of this shop. The exact
/// stored version and its contract are reinstalled; app data follows the rollback steps the current
/// version declares for that target, so a version without them cannot be rolled back to.
pub async fn rollback(a: &Platform, t: &str, app: &str, version: &str) -> Result<Value> {
    let mut tx = a.db.begin_privileged().await?;
    crate::store::locks::tenant_scope(&mut tx, t).await?;
    let current = package_contracts::of_app(&mut *tx, t, app).await?;
    let (m, contract) = package_contracts::version(&mut tx, t, app, version).await?;
    let steps = current
        .as_ref()
        .and_then(|c| c.rollbacks.iter().find(|r| r.to_version == version))
        .ok_or(conflict(
            "The installed version declares no rollback to this version",
        ))?
        .clone();
    let result = install_inner(
        a.rules,
        a.commerce,
        &mut tx,
        t,
        m,
        true,
        contract.as_ref(),
        Some(&steps),
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}
/// Installs or upgrades a package inside the transaction of the caller and keeps its version history.
pub async fn install_tx(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    install_inner(rules, commerce, tx, t, m, true, None, None).await
}
/// Installs a mutable package into a personal preview environment, without version history.
pub async fn install_preview(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    let private = veyndo_core::store::previews::is_personal(tx, t).await?;
    if !private {
        return Err(bad(
            "Mutable app preview requires a personal preview environment",
        ));
    }
    install_inner(rules, commerce, tx, t, m, false, None, None).await
}
#[allow(clippy::too_many_arguments)]
async fn install_inner(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
    versions: bool,
    contract: Option<&Contract>,
    rollback: Option<&Rollback>,
) -> Result<Value> {
    let value = json!(m);
    let result = veyndo_app_storage::installation::install(
        tx,
        &veyndo_app_storage::installation::Package {
            scope: veyndo_app_storage::Scope {
                tenant: t,
                app: &m.id,
            },
            version: &m.version,
            manifest: &value,
            versions,
        },
        &Host {
            tenant: t,
            manifest: &m,
            rules,
            commerce,
            rollback,
        },
    )
    .await
    .map_err(error)?;
    // Still under the tenant installation lock of the shared sequence: wire ports, keep the version's contract.
    package_contracts::install(
        tx,
        t,
        &m,
        contract,
        &process_config::get().publishers,
        versions,
    )
    .await?;
    Ok(result)
}
