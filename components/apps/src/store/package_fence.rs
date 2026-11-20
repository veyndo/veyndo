//! Shared package fence prevents native record or document writes racing contract upgrades.
use crate::domain::approval;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad, conflict};

/// Locks the installed package and requires it to be the one the caller validated against.
pub async fn lock_package_write(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    let installed = veyndo_app_storage::packages::lock(
        tx,
        veyndo_app_storage::Scope {
            tenant: t,
            app: &m.id,
        },
    )
    .await?;
    let current: Manifest = serde_json::from_value(
        installed.ok_or(conflict("App is no longer active; reopen its view"))?,
    )
    .map_err(|_| bad("Invalid installed app contract"))?;
    if approval::canonical_digest(&current) != approval::canonical_digest(m) {
        return Err(conflict(
            "App package changed during this write; reopen its view",
        ));
    }
    Ok(())
}
