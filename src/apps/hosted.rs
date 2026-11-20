//! Hosted-app installation lives in the apps component; this module adds the connected frontends and their local answers.
use super::*;
use sqlx::{Postgres, Transaction};
use veyndo_app_platform::store::hosted_apps;

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
    let rows=sqlx::query("SELECT alias,channel,experience_alias,revision,app_id FROM hosted_frontends WHERE tenant=$1 ORDER BY alias")
        .bind(tenant).fetch_all(&a.db).await?;
    Ok(rows.iter().map(|r| {
        let alias:String=r.get("alias"); let experience:String=r.get("experience_alias");
        json!({"alias":alias,"channel":r.get::<String,_>("channel"),"experienceAlias":experience,"revision":r.get::<i64,_>("revision"),"appId":r.get::<Option<String>,_>("app_id"),"url":crate::shop_domains::links(&alias)["storefrontUrl"],"editorUrl":crate::shop_domains::frontend_editor::url(&experience,env::var("HOSTED_FRONTEND_EDITOR_URL").ok().as_deref())})
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
