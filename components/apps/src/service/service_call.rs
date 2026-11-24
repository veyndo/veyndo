//! Operator-configured service egress: no URL or credential comes from a package, merchant, model or event.
use crate::domain::{approval, service_policy};
use crate::ports::Platform;
use crate::service::{admission, egress};
use crate::store::packages::package;
use crate::store::{secrets, settings};
use serde_json::Value;
use veyndo_core::shop_scope::parent;
use veyndo_core::{Error, Result, StatusCode, bad, conflict, http_limits, process_config};
use veyndo_kernel::verified_kernel;

/// Calls the operator-configured service of an app.
pub async fn service_call(a: &Platform, t: &str, id: &str, path: &str, v: &Value) -> Result<Value> {
    service_call_pinned(a, t, id, path, v, None).await
}
/// Calls the service only if the installed package still has the digest the caller admitted.
pub async fn service_call_pinned(
    a: &Platform,
    t: &str,
    id: &str,
    path: &str,
    v: &Value,
    digest: Option<&str>,
) -> Result<Value> {
    if parent(a, t).await?.is_some() {
        return Err(bad("External services are disabled in private sandboxes"));
    }
    // Background collectors/flows have no HTTP admission middleware. Recheck the
    // current live-shop status at the shared egress boundary as well.
    let status: Option<String> = veyndo_core::tenants::status(&a.db, t).await?;
    if !verified_kernel::shop_request_admissible(
        status.as_deref() == Some("active"),
        status.as_deref() == Some("paused"),
        false,
        false,
    ) {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Shop is unavailable for external app calls".into(),
        ));
    }
    // No URL or credential comes from the manifest, merchant, model or event payload.
    let _admission = admission::admit(a, t, id).await?;
    if v.to_string().len() > 65536 {
        return Err(bad("App request exceeds limit"));
    }
    let configured = &process_config::get().services;
    let config = &configured[id];
    let manifest = package(a, t, id, true).await?;
    if digest.is_some_and(|d| d != approval::canonical_digest(&manifest)) {
        return Err(conflict("App package changed before service dispatch"));
    }
    if !approval::approved(&manifest, config) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Package is not approved for this operator service".into(),
        ));
    }
    let url = config["url"].as_str().ok_or(Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "App service is not configured".into(),
    ))?;
    let parsed = reqwest::Url::parse(url).map_err(|_| bad("Invalid service URL"))?;
    if !service_policy::allowed(&parsed, &process_config::get().private_service_origins) {
        return Err(bad(
            "Service requires HTTPS, loopback or an explicitly approved private origin",
        ));
    }
    let service_token = secrets::resolve(a, t, &manifest, "service")
        .await?
        .unwrap_or_else(|| config["token"].as_str().unwrap_or("").to_owned());
    let mut body = v.clone();
    if !manifest.settings.is_empty() {
        let private_action = path
            .strip_prefix("actions/")
            .and_then(|name| manifest.actions.iter().find(|action| action.name == name))
            .is_some_and(|action| !action.public);
        if private_action {
            let values = settings::resolve(a, t, &manifest).await?;
            if manifest.settings.iter().any(|f| {
                f.required
                    && values
                        .get(&f.key)
                        .is_none_or(|v| v.is_null() || v.as_str().is_some_and(str::is_empty))
            }) {
                return Err(bad(
                    "Complete this app's settings in Apps before running this action",
                ));
            }
            body.as_object_mut()
                .ok_or(bad("Configured service actions require object input"))?
                .insert("_appSettings".into(), values);
        }
    }
    // Installed configuration shares the transport budget with caller input.
    if body.to_string().len() > 65536 {
        return Err(bad(
            "App request including installed settings exceeds 64 KiB",
        ));
    }
    let response = egress::client(&parsed, &process_config::get().private_service_origins)
        .await?
        .post(format!("{}/{path}", url.trim_end_matches('/')))
        .timeout(std::time::Duration::from_secs(5))
        .bearer_auth(service_token)
        .header("x-tenant", t)
        .json(&body)
        .send()
        .await
        .map_err(|_| Error(StatusCode::BAD_GATEWAY, "App service unavailable".into()))?;
    if !response.status().is_success() {
        return Err(Error(
            StatusCode::BAD_GATEWAY,
            "App service rejected request".into(),
        ));
    }
    if response.content_length().is_some_and(|n| n > 65536) {
        return Err(bad("App response exceeds limit"));
    }
    http_limits::json_body(response).await
}
