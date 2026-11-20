//! Atomic installation and additive schema upgrades; immutable version digests preserve history.
use super::*;
pub(crate) use veyndo_app_platform::store::packages::{active_manifests, package, package_on};
pub(crate) async fn install(a: &App, t: &str, m: Manifest) -> Result<Value> {
    let mut tx = a.db.begin_privileged().await?;
    let result = install_tx(&mut tx, t, m).await?;
    tx.commit().await?;
    Ok(result)
}
pub(crate) async fn install_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    install_inner(tx, t, m, true).await
}
pub(crate) async fn install_preview(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: Manifest,
) -> Result<Value> {
    let private: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shop_environments WHERE tenant=$1 AND preview_owner IS NOT NULL)").bind(t).fetch_one(&mut **tx).await?;
    if !private {
        return Err(bad(
            "Mutable app preview requires a personal preview environment",
        ));
    }
    install_inner(tx, t, m, false).await
}
async fn install_inner(
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
        &package_install::Host {
            tenant: t,
            manifest: &m,
        },
    )
    .await
    .map_err(package_install::error)
}
