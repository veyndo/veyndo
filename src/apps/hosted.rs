//! Hosted-app installation lives in the apps component; this module adds the connected frontends and their local answers.
use super::{data, install_host};
use crate::foundation::App;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use std::env;
use veyndo_app_platform::store::hosted_apps;
use veyndo_core::Result;
use veyndo_core::conflict;
use veyndo_core::document::view;

pub(crate) async fn ensure(
    tx: &mut Transaction<'_, Postgres>,
    tenant: &str,
    id: &str,
) -> Result<()> {
    hosted_apps::ensure(&data::BUNDLED, &install_host::COMMERCE, tx, tenant, id).await
}

/// One hosted frontend bound to a channel, as the app list and frontend bindings show it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostedConnection {
    alias: String,
    channel: String,
    experience_alias: String,
    revision: i64,
    pub(crate) app_id: Option<String>,
    url: Value,
    editor_url: Option<String>,
}

/// The storefront address of a shop's links.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ShopLinks {
    storefront_url: Value,
}

pub(crate) async fn connections(a: &App, tenant: &str) -> Result<Vec<HostedConnection>> {
    let rows = veyndo_app_platform::store::app_host_reads::connections(a, tenant).await?;
    Ok(rows
        .into_iter()
        .map(|c| HostedConnection {
            url: view::<ShopLinks>(&crate::shop_domains::links(&c.alias)).storefront_url,
            editor_url: crate::shop_domains::frontend_editor::url(
                &c.experience,
                env::var("HOSTED_FRONTEND_EDITOR_URL").ok().as_deref(),
            ),
            alias: c.alias,
            channel: c.channel,
            experience_alias: c.experience,
            revision: c.revision,
            app_id: c.app,
        })
        .collect())
}

/// Existing action contract resolves managed connections locally, without inventing a second generator.
pub(crate) async fn action(a: &App, tenant: &str, id: &str, name: &str) -> Result<Option<Value>> {
    if id != "storyfront" {
        return Ok(None);
    }
    let bindings: Vec<HostedConnection> = connections(a, tenant)
        .await?
        .into_iter()
        .filter(|f| f.app_id.as_deref() == Some(id))
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

#[cfg(test)]
mod tests {
    use super::HostedConnection;
    use serde_json::json;

    #[test]
    fn hosted_connection_keeps_the_literal_shape() {
        let app: Option<String> = Some("storyfront".into());
        let editor: Option<String> = None;
        let old = json!({"alias":"demo-shop","channel":"web","experienceAlias":"demo-exp","revision":4,"appId":app,"url":"https://demo-shop.example.test/","editorUrl":editor});
        let new = json!(HostedConnection {
            alias: "demo-shop".into(),
            channel: "web".into(),
            experience_alias: "demo-exp".into(),
            revision: 4,
            app_id: app,
            url: json!("https://demo-shop.example.test/"),
            editor_url: editor,
        });
        assert_eq!(new, old);
    }
}
