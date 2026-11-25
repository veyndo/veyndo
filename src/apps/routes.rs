//! Tenant-scoped package lifecycle, generated data endpoints and a shared action adapter.
use super::{
    Manifest, approval, asset_surfaces, configure, consent, core_callbacks, core_documents,
    credentials, evidence_routes, gateway, hosted, install, jobs, observability, record_history,
    secrets, settings, surface_grants, surfaces, ui_bundles, ui_documents,
};
use crate::auth::SecureRoutes;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, commerce, payments};
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use veyndo_app_platform::http::actions::{app_action, entity_list, entity_save, slots};
use veyndo_app_platform::http::packages;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::request_context::RequestContext;
pub(crate) fn app_router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/agent/apps/search",
            &[("POST", "apps.read")],
            post(packages::search),
        )
        .secure_route(
            "/api/agent/apps/read",
            &[("POST", "apps.read")],
            post(packages::read),
        )
        .merge(super::webhooks::router())
        .merge(evidence_routes::router())
        .merge(record_history::router())
        .merge(observability::router())
        .merge(surfaces::router())
        .merge(surface_grants::router())
        .merge(ui_bundles::router())
        .merge(ui_documents::router())
        .merge(credentials::router())
        .merge(consent::router())
        .merge(secrets::router())
        .merge(settings::router())
        .merge(core_callbacks::router())
        .merge(core_documents::router())
        .merge(asset_surfaces::router())
        .merge(jobs::router())
        .route("/store-api/apps/analytics.js", get(packages::analytics_sdk))
        .secure_route(
            "/api/apps",
            &[("GET", "apps.read"), ("POST", "apps.manage")],
            get(app_list).post(app_install),
        )
        .secure_route(
            "/api/apps/{id}",
            &[("PUT", "apps.manage")],
            axum::routing::put(packages::state),
        )
        .secure_route(
            "/api/apps/{id}/actions/{action}",
            &[("POST", "read")],
            post(app_action),
        )
        .route("/store-api/apps/{id}/actions/{action}", post(app_action))
        .secure_route(
            "/api/apps/{id}/entities/{entity}",
            &[("GET", "catalog.read"), ("POST", "apps.manage")],
            get(entity_list).post(entity_save),
        )
        .route("/store-api/apps/slots", get(slots))
        .route("/store-api/apps/{id}/configure", post(configure_action))
}
/// The package row fields the app list reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PackageRow {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    manifest: Value,
}

/// The links the app list adds to each package row.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageLinks<'a> {
    ui_url: Option<String>,
    connections: Vec<&'a hosted::HostedConnection>,
    managed_by: Option<&'static str>,
}

async fn app_list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let (config, _) = commerce::config(&a, &t).await?;
    let connections = hosted::connections(&a, &t).await?;
    let rows = veyndo_app_platform::store::app_host_reads::packages(&a, &t).await?;
    let packages: Vec<Value> = rows
        .into_iter()
        .map(|mut r| {
            let package: PackageRow = view(&r);
            let id = package.id.unwrap_or_default();
            let connected: Vec<&hosted::HostedConnection> = connections
                .iter()
                .filter(|f| f.app_id.as_deref() == Some(id.as_str()))
                .collect();
            merge(
                &mut r,
                PackageLinks {
                    ui_url: serde_json::from_value::<Manifest>(package.manifest)
                        .ok()
                        .filter(|m| {
                            approval::approved(m, &crate::runtime_config::get().services[&id])
                        })
                        .and_then(|_| gateway::ui_url(&id)),
                    managed_by: if connected.is_empty() {
                        None
                    } else {
                        Some("experience")
                    },
                    connections: connected,
                },
            )?;
            Ok(r)
        })
        .collect::<Result<_>>()?;
    Ok(Json(json!({
        "apiVersion":"1", "mainLocale":config.main_locale, "packages":packages,
        "builtIns":["migration","site_builder","site_forms","marketplaces","engraving","paypal","shopware_payments","storyfront","google_analytics","gmail","slack","email","vercel","klaviyo","hubspot","contentful","algolia"],
        "serviceExecution":"digest-approved external services and bounded typed Wasm commerce hooks"
    })))
}
/// The JSON fields `app_install` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AppInstallInput {
    built_in: Value,
}

async fn app_install(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: AppInstallInput = view(&v);
    auth::permit(&h, "users")?;
    let t = merchant(&a, &h)?;
    let m = consent::decode(&v)?;
    consent::approved(&h, &m, &v)?;
    let result = install(&a, &t, m).await?;
    if input.built_in == "paypal" {
        let mut tx = a.db.begin().await?;
        let mut config: commerce::Settings = serde_json::from_value(
            veyndo_shop::store::settings::basis_for_update(&mut tx, &t).await?,
        )
        .map_err(|_| bad("Invalid settings"))?;
        let live = payments::environment() == "live";
        let method = if live {
            "paypal-live"
        } else {
            "paypal-sandbox"
        };
        if !config.payments.iter().any(|p| p.id == method) {
            config.payments.push(commerce::Payment {
                id: method.into(),
                name: if live { "PayPal" } else { "PayPal Sandbox" }.into(),
                active: payments::account(&t).is_ok(),
                business_only: false,
                translations: HashMap::new(),
                countries: Vec::new(),
                restricted_countries: false,
                mode: "app".into(),
                provider: None,
                provider_method: None,
                capture_mode: None,
            });
            veyndo_shop::store::settings::replace_basis(&mut tx, &t, &json!(config)).await?;
        }
        tx.commit().await?;
    }
    Ok(Json(result))
}
async fn configure_action(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(configure(&a, &h, &id, &v).await?))
}

#[cfg(test)]
mod tests {
    use super::PackageLinks;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn package_links_keep_the_indexed_shape() {
        let row = json!({"id":"storyfront","manifest":{"id":"storyfront"},"enabled":true});
        for (ui, connected) in [
            (Some("https://ui.example.test/".to_string()), false),
            (None, false),
        ] {
            let mut old = row.clone();
            let none: Vec<serde_json::Value> = vec![];
            old["uiUrl"] = json!(ui);
            old["connections"] = json!(none);
            old["managedBy"] = json!(if connected { Some("experience") } else { None });
            let mut new = row.clone();
            merge(
                &mut new,
                PackageLinks {
                    ui_url: ui,
                    connections: vec![],
                    managed_by: None,
                },
            )
            .unwrap();
            assert_eq!(new, old);
        }
    }
}
