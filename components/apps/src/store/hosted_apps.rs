//! Persist hosted-app dependencies through the existing package installer; no private renderer or parallel registry.
use crate::ports::{CommerceInstall, RecordRules};
use crate::store::package_install::install_tx;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use veyndo_core::{Result, bad};

/// The bundled package of the hosted storefront experience.
pub const STORYFRONT: &str = include_str!("../../../../extensions/apps/storyfront/manifest.json");

/// Installs or reactivates the app a connected frontend depends on.
pub async fn ensure(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    id: &str,
) -> Result<()> {
    if id != "storyfront" {
        return Err(bad("Unknown hosted app"));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,7))")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let active: Option<bool> =
        sqlx::query_scalar("SELECT active FROM app_packages WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(tenant)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
    match active {
        None => {
            install_tx(
                rules,
                commerce,
                tx,
                tenant,
                serde_json::from_str(STORYFRONT).map_err(|_| bad("Invalid built-in"))?,
            )
            .await?;
        }
        Some(false) => {
            sqlx::query(
                "UPDATE app_packages SET active=true,revision=revision+1 WHERE tenant=$1 AND id=$2",
            )
            .bind(tenant)
            .bind(id)
            .execute(&mut **tx)
            .await?;
            veyndo_core::outbox::publish_on(
                &mut **tx,
                tenant,
                "app.activated",
                &json!({"app":id,"reason":"hosted_frontend"}),
            )
            .await?;
        }
        Some(true) => {}
    }
    Ok(())
}

/// Runs after shared-storage consolidation under the migration lock; normal reads never install apps.
pub async fn backfill(
    rules: &dyn RecordRules,
    commerce: &dyn CommerceInstall,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<()> {
    let tenants: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT tenant FROM hosted_frontends WHERE app_id='storyfront' ORDER BY tenant",
    )
    .fetch_all(&mut **tx)
    .await?;
    for tenant in tenants {
        ensure(rules, commerce, tx, &tenant, "storyfront").await?;
    }
    sqlx::query("ALTER TABLE hosted_frontends VALIDATE CONSTRAINT hosted_frontends_app_fk")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
