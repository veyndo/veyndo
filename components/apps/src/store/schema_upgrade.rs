//! Native field/content-language authority for shared bounded schema migration on the original installation transaction.
use crate::domain::{field_values, model_contract};
use crate::store::languages;
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::schema_upgrade::{self as shared, UpgradeError, Validation};
use veyndo_core::{Error, Result, bad, conflict};
struct Host<'a> {
    tenant: &'a str,
}
impl<'db> Validation<'db> for Host<'_> {
    type Error = Error;
    fn fields<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
        entity: &'a Entity,
        fields: &'a Value,
    ) -> veyndo_app_storage::installation::HookFuture<'a, Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            field_values::validate(entity, fields)?;
            languages::validate_languages(tx, self.tenant, entity, fields).await
        })
    }
}
/// Replaces the recovery snapshot kept for `version` when a shop returns to a version it already
/// migrated to (after a rollback): the shared upgrade keeps one snapshot per target version, and the
/// newer one is taken from the current records by the migration that follows on this transaction.
pub async fn retire_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    app: &str,
    version: &str,
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM app_schema_history WHERE tenant=$1 AND app=$2 AND to_version=$3",
        tenant,
        app,
        version
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
/// Migrates stored records to the next package version on the installation transaction.
pub async fn upgrade(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    old: &Manifest,
    manifest: &Manifest,
) -> Result<()> {
    shared::upgrade(
        tx,
        veyndo_app_storage::Scope {
            tenant,
            app: &manifest.id,
        },
        old,
        manifest,
        &Host { tenant },
    )
    .await
    .map_err(|error| match error {
        UpgradeError::Database(error) => error.into(),
        UpgradeError::Model(error) => model_contract::error(error),
        UpgradeError::Invalid(message) => bad(message),
        UpgradeError::Conflict(message) => conflict(message),
        UpgradeError::Host(error) => error,
    })
}
