//! Tenant-scoped package lifecycle, generated data endpoints and a shared action adapter.
use super::*;
use veyndo_app_platform::http::actions::{app_action, entity_list, entity_save, slots};
use veyndo_app_platform::http::packages;
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
async fn app_list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let (config, _) = commerce::config(&a, &t).await?;
    let connections = hosted::connections(&a, &t).await?;
    let rows=sqlx::query("SELECT id,version,manifest,active,revision,digest FROM app_packages WHERE tenant=$1 ORDER BY id").bind(t).fetch_all(&a.db).await?;
    let packages: Vec<Value> = rows
        .iter()
        .map(|r| {
            let id: String = r.get("id");
            let connected: Vec<&Value> = connections.iter().filter(|f| f["appId"] == id).collect();
            json!({
                "id":id, "version":r.get::<String,_>("version"),
                "manifest":r.get::<Value,_>("manifest"), "uiUrl":serde_json::from_value::<Manifest>(r.get("manifest")).ok().filter(|m|approval::approved(m,&crate::runtime_config::get().services[&id])).and_then(|_|gateway::ui_url(&id)),
                "active":r.get::<bool,_>("active"), "revision":r.get::<i64,_>("revision"),
                "digest":r.get::<String,_>("digest"), "connections":connected,
                "managedBy":if connected.is_empty() { None } else { Some("experience") }
            })
        })
        .collect();
    Ok(Json(json!({
        "apiVersion":"1", "mainLocale":config.main_locale, "packages":packages,
        "builtIns":["migration","site_builder","site_forms","marketplaces","engraving","paypal","shopware_payments","storyfront","google_analytics","gmail","slack","email","vercel","klaviyo","hubspot","contentful","algolia"],
        "serviceExecution":"digest-approved external services and bounded typed Wasm commerce hooks"
    })))
}
async fn app_install(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    auth::permit(&h, "users")?;
    let t = merchant(&a, &h)?;
    let m = consent::decode(&v)?;
    consent::approved(&h, &m, &v)?;
    let result = install(&a, &t, m).await?;
    if v["builtIn"] == "paypal" {
        let mut tx = a.db.begin().await?;
        let row = sqlx::query("SELECT data FROM commerce_settings WHERE tenant=$1 FOR UPDATE")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
        let mut config: commerce::Settings =
            serde_json::from_value(row.get("data")).map_err(|_| bad("Invalid settings"))?;
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
            sqlx::query("UPDATE commerce_settings SET data=$1,revision=revision+1 WHERE tenant=$2")
                .bind(json!(config))
                .bind(&t)
                .execute(&mut *tx)
                .await?;
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
