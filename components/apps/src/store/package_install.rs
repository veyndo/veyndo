//! Atomic installation and additive schema upgrades; required hooks preserve compiler/trust, commerce, schema, default-record and schedule admission in the shared transaction.
use crate::domain::manifest_validation::validate;
use crate::domain::{approval, field_values};
use crate::ports::{CommerceInstall, Platform, RecordRules};
use crate::service::configuration_runtime::validate_fee;
use crate::service::package_verification::verify_package;
use crate::store::{
    commerce_hooks, dependencies, managed_data, native_lifecycle, records, schedules,
    schema_upgrade, storage_usage,
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
            let version = |s: &str| {
                s.split('.')
                    .map(|v| v.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>()
            };
            if version(&self.manifest.version) < version(&old.version) {
                return Err(conflict(
                    "Downgrade requires an explicit compatible migration",
                ));
            }
            schema_upgrade::upgrade(tx, self.tenant, &old, self.manifest).await
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
/// The response for a refused or failed installation.
pub fn error(error: InstallError<Error>) -> Error {
    match error {
        InstallError::Database(error) => error.into(),
        InstallError::Invalid(message) => bad(message),
        InstallError::Conflict(message) => conflict(message),
        InstallError::Hook(error) => error,
    }
}
/// Installs or upgrades a package in its own privileged transaction.
pub async fn install(a: &Platform, t: &str, m: Manifest) -> Result<Value> {
    let mut tx = a.db.begin_privileged().await?;
    let result = install_tx(a.rules, a.commerce, &mut tx, t, m).await?;
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
    install_inner(rules, commerce, tx, t, m, true).await
}
/// Installs a mutable package into a personal preview environment, without version history.
pub async fn install_preview(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    let private = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NOT NULL) AS "private!""#,
        t
    )
    .fetch_one(&mut **tx)
    .await?;
    if !private {
        return Err(bad(
            "Mutable app preview requires a personal preview environment",
        ));
    }
    install_inner(rules, commerce, tx, t, m, false).await
}
async fn install_inner(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
    versions: bool,
) -> Result<Value> {
    let value = json!(m);
    veyndo_app_storage::installation::install(
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
        },
    )
    .await
    .map_err(error)
}
