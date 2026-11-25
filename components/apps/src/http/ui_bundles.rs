//! Operator-pinned, bounded self-contained UI bundles use the same actor/context grant as app actions.
use crate::domain::surfaces::surface_url;
use crate::domain::{approval, native_views};
use crate::ports::Platform;
use crate::service::admission::admit;
use crate::service::ui_bundles::fetch;
use crate::store::surface_grants;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_core::document::view;
use veyndo_core::shop_scope::tenant;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, process_config};

/// The UI bundle digests an operator pinned for one app service, by UI path.
#[derive(serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct UiPins {
    #[serde(deserialize_with = "veyndo_core::document::lenient")]
    pub ui_digests: serde_json::Map<String, Value>,
}

/// Returns the operator-pinned UI bundle of a surface the caller holds a grant for.
pub async fn load(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path((id, surface)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let (m, s, _, _) = surface_grants::load(&a, &h, &id, &surface, &v).await?;
    let services = &process_config::get().services;
    let config = &services[&id];
    if !approval::approved(&m, config) || native_views::is_native(&m, &s) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "External UI package is not approved".into(),
        ));
    }
    let pins: UiPins = view(config);
    let digest = pins
        .ui_digests
        .get(&s.ui_path)
        .and_then(Value::as_str)
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "App UI bundle requires an operator-approved SHA-256 digest".into(),
        ))?;
    let url = reqwest::Url::parse(
        &surface_url(&id, &s.ui_path, services).ok_or(bad("App UI URL not configured"))?,
    )
    .map_err(|_| bad("Invalid UI URL"))?;
    let _admission = admit(&a, &tenant(&h)?, &id).await?;
    let html = fetch(url, digest).await?;
    Ok(Json(
        json!({"html":html,"digest":digest,"documentUrl":format!("/app-ui/{id}/{digest}"),"sandbox":"opaque-origin; gateway-only actions"}),
    ))
}
