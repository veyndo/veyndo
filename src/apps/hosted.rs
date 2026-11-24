//! Hosted-app installation lives in the apps component; this module adds the connected frontends and their local answers.
use super::{data, install_host};
use crate::App;
use serde_json::Value;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use std::env;
use veyndo_app_platform::store::hosted_apps;
use veyndo_core::Result;
use veyndo_core::conflict;

pub(crate) async fn ensure(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    id: &str,
) -> Result<()> {
    hosted_apps::ensure(&data::BUNDLED, &install_host::COMMERCE, tx, tenant, id).await
}

/// Runs after shared-storage consolidation under the migration lock; normal reads never install apps.
pub(crate) async fn backfill(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    hosted_apps::backfill(&data::BUNDLED, &install_host::COMMERCE, tx).await
}

pub(crate) async fn connections(a: &App, tenant: &str) -> Result<Vec<Value>> {
    let rows = veyndo_app_platform::store::app_host_reads::connections(a, tenant).await?;
    Ok(rows.into_iter().map(|c| {
        json!({"alias":c.alias,"channel":c.channel,"experienceAlias":c.experience,"revision":c.revision,"appId":c.app,"url":crate::shop_domains::links(&c.alias)["storefrontUrl"],"editorUrl":crate::shop_domains::frontend_editor::url(&c.experience,env::var("HOSTED_FRONTEND_EDITOR_URL").ok().as_deref())})
    }).collect())
}

/// Existing action contract resolves managed connections locally, without inventing a second generator.
pub(crate) async fn action(a: &App, tenant: &str, id: &str, name: &str) -> Result<Option<Value>> {
    if id != "storyfront" {
        return Ok(None);
    }
    let bindings: Vec<Value> = connections(a, tenant)
        .await?
        .into_iter()
        .filter(|f| f["appId"] == id)
        .collect();
    if bindings.is_empty() {
        return Ok(None);
    }
    if name == "status" {
        return Ok(Some(
            json!({"managedBy":"experience","frontends":bindings,"status":"connected"}),
        ));
    }
    if name == "generate" {
        return Err(conflict(
            "Use the connected Storyfront editor to generate and publish this Experience",
        ));
    }
    Ok(None)
}
