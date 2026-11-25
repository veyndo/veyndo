//! Operator-only platform inference administration: optimistic revision, encrypted write-only keys and an audit without secrets.
use crate::service::models::Models;
use crate::store::platform_ai as store;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_cognition::inference::{settings, sovereign};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict};
use veyndo_kernel::verified_kernel;
use veyndo_shop::http::operator::actor;

/// The platform inference configuration as operators see it: stored overrides on the environment defaults, which keys are stored, and the effective providers.
pub async fn get(m: Models<'_>, h: &RequestContext) -> Result<Value> {
    actor(h)?;
    snapshot(m).await
}
async fn snapshot(m: Models<'_>) -> Result<Value> {
    let a = m.core;
    let row = store::read(&a.db).await?;
    let secrets = &row.secrets;
    let stored = &row.data;
    let visible = visible(m.inference.environment_configuration(), stored)?;
    Ok(json!({"revision":row.revision,"settings":visible,
        "updatedAt":row.updated,"encryptedStorageReady":settings::encryption_ready(),
        "keyStored":{"ollama":secrets.get("ollama").is_some(),"openai":secrets.get("openai").is_some(),"anthropic":secrets.get("anthropic").is_some()},
        "effective":m.inference.public_providers().await.ok(),"inheritance":"all tenants; replica refresh within 5 seconds"}))
}
/// The environment configuration with the stored default provider, provider routes and sovereignty switch laid over it.
fn visible(mut visible: Value, stored: &Value) -> Result<Value> {
    if let Some(provider) = stored.get("defaultProvider") {
        merge(&mut visible, json!({ "defaultProvider": provider }))?;
    }
    let configured = view::<Configuration>(stored);
    let mut providers = view::<Configuration>(&visible).providers;
    for id in ["ollama", "openai", "anthropic"] {
        if let Some(provider) = configured.providers.get(id) {
            providers.insert(id.to_owned(), provider.clone());
        }
    }
    merge(
        &mut visible,
        json!({ "providers": providers, "sovereign": configured.sovereign == true }),
    )?;
    Ok(visible)
}
/// A platform inference configuration save.
#[derive(Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct PlatformSettings {
    #[serde(deserialize_with = "lenient")]
    settings: SettingsInput,
    #[serde(deserialize_with = "lenient")]
    revision: Option<u64>,
    #[serde(deserialize_with = "lenient")]
    clear_keys: serde_json::Map<String, Value>,
    #[serde(deserialize_with = "lenient")]
    keys: serde_json::Map<String, Value>,
}

/// The stored or environment configuration as the snapshot and checks read it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Configuration {
    #[serde(deserialize_with = "lenient")]
    providers: serde_json::Map<String, Value>,
    sovereign: Value,
}

/// The saved platform configuration document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredConfiguration<'a> {
    default_provider: &'a str,
    providers: StoredRoutes,
    sovereign: bool,
}

/// The saved route of every supported provider, in the order the operator form lists them.
#[derive(Serialize)]
struct StoredRoutes {
    ollama: StoredRoute,
    openai: StoredRoute,
    anthropic: StoredRoute,
}

/// One saved provider route.
#[derive(Serialize)]
struct StoredRoute {
    model: String,
    endpoint: String,
    enabled: bool,
}

/// The submitted settings: default provider, per-provider routes and the sovereignty switch.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SettingsInput {
    #[serde(deserialize_with = "lenient")]
    default_provider: Option<String>,
    #[serde(deserialize_with = "lenient")]
    providers: serde_json::Map<String, Value>,
    #[serde(deserialize_with = "lenient")]
    sovereign: Option<bool>,
}

/// One provider route.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProviderSettings {
    #[serde(deserialize_with = "lenient")]
    model: Option<String>,
    #[serde(deserialize_with = "lenient")]
    endpoint: Option<String>,
    #[serde(deserialize_with = "lenient")]
    enabled: Option<bool>,
}

/// Saves the platform inference configuration at its revision; keys are sealed write-only and the audit never holds secrets.
pub async fn save(m: Models<'_>, h: &RequestContext, v: &Value) -> Result<Value> {
    let a = m.core;
    let actor = actor(h)?;
    let input: PlatformSettings = view(v);
    let provider = input
        .settings
        .default_provider
        .as_deref()
        .filter(|v| ["ollama", "openai", "anthropic"].contains(v))
        .ok_or(bad("Choose a supported default provider"))?;
    let mut routes = Vec::with_capacity(3);
    let mut ollama_endpoint = String::new();
    for id in ["ollama", "openai", "anthropic"] {
        let row: ProviderSettings = input
            .settings
            .providers
            .get(id)
            .map(view)
            .unwrap_or_default();
        let model = row
            .model
            .as_deref()
            .filter(|s| {
                !s.is_empty() && s.len() <= 128 && s.is_ascii() && !s.chars().any(char::is_control)
            })
            .ok_or(bad("Invalid model identifier"))?;
        let endpoint = row
            .endpoint
            .as_deref()
            .filter(|s| {
                settings::valid_endpoint(s)
                    || view::<Configuration>(&m.inference.environment_configuration())
                        .providers
                        .get(id)
                        .map(view::<ProviderSettings>)
                        .and_then(|p| p.endpoint)
                        .as_deref()
                        == Some(s)
            })
            .ok_or(bad(
                "Provider endpoint must be HTTPS without credentials, query or fragment",
            ))?;
        let enabled = row.enabled.ok_or(bad("Provider enabled state required"))?;
        if id == provider && !enabled {
            return Err(bad("The default provider must be enabled"));
        }
        let endpoint = endpoint.trim_end_matches('/');
        if id == "ollama" {
            ollama_endpoint = endpoint.to_owned();
        }
        routes.push(StoredRoute {
            model: model.to_owned(),
            endpoint: endpoint.to_owned(),
            enabled,
        });
    }
    let sovereign = input.settings.sovereign.unwrap_or(false);
    let [ollama, openai, anthropic]: [StoredRoute; 3] = routes
        .try_into()
        .map_err(|_| bad("Provider routes required"))?;
    let data = json!(StoredConfiguration {
        default_provider: provider,
        providers: StoredRoutes {
            ollama,
            openai,
            anthropic
        },
        sovereign,
    });
    // Refuse a switch that would only fail closed later: every configured model route must be self-operated.
    if sovereign || sovereign::locked() {
        let blockers = m.inference.sovereign_blockers(&ollama_endpoint);
        if !blockers.is_empty() {
            return Err(Error(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "Sovereign AI mode needs self-hosted endpoints for: {}",
                    blockers.join(", ")
                ),
            ));
        }
    }
    let mut tx = a.db.begin().await?;
    let row = store::lock(&mut *tx).await?;
    if !verified_kernel::revision_admissible(
        input
            .revision
            .filter(|r| *r > 0)
            .ok_or(bad("Positive revision required"))?,
        row.revision as u64,
    ) {
        return Err(conflict("AI configuration changed; reload before saving"));
    }
    let mut secrets = row.secrets;
    for id in ["ollama", "openai", "anthropic"] {
        if input.clear_keys.get(id).and_then(Value::as_bool) == Some(true) {
            secrets.as_object_mut().unwrap().remove(id);
        }
        if let Some(key) = input
            .keys
            .get(id)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if key.len() > 4096 || key.chars().any(char::is_control) {
                return Err(bad("Invalid provider key"));
            }
            merge(
                &mut secrets,
                json!({ id: settings::seal(id, key).map_err(bad)? }),
            )?;
        }
    }
    store::save(
        &mut tx,
        &data,
        &secrets,
        actor,
        &json!({"defaultProvider":provider,"sovereign":sovereign}),
    )
    .await?;
    tx.commit().await?;
    m.inference.invalidate().await;
    snapshot(m).await
}

#[cfg(test)]
mod tests {
    use super::{StoredConfiguration, StoredRoute, StoredRoutes, visible};
    use serde_json::json;

    fn route(model: &str, endpoint: &str, enabled: bool) -> StoredRoute {
        StoredRoute {
            model: model.into(),
            endpoint: endpoint.into(),
            enabled,
        }
    }

    #[test]
    fn stored_configuration_keeps_the_indexed_shape() {
        let mut old = json!({"defaultProvider":"openai","providers":{}});
        old["providers"]["ollama"] = json!({"model":"m1","endpoint":"http://o","enabled":true});
        old["providers"]["openai"] = json!({"model":"m2","endpoint":"https://a","enabled":true});
        old["providers"]["anthropic"] =
            json!({"model":"m3","endpoint":"https://b","enabled":false});
        old["sovereign"] = json!(false);
        let new = json!(StoredConfiguration {
            default_provider: "openai",
            providers: StoredRoutes {
                ollama: route("m1", "http://o", true),
                openai: route("m2", "https://a", true),
                anthropic: route("m3", "https://b", false),
            },
            sovereign: false,
        });
        assert_eq!(new.to_string(), old.to_string());
    }

    #[test]
    fn visible_configuration_keeps_the_indexed_shape() {
        let environment = json!({"defaultProvider":"ollama","providers":{
            "ollama":{"endpoint":"http://o","model":"l","enabled":true},
            "openai":{"endpoint":"https://a","model":"g","enabled":true},
            "anthropic":{"endpoint":"https://b","model":"c","enabled":true}}});
        for stored in [
            json!({}),
            json!({"defaultProvider":"openai","providers":{"openai":{"model":"x"}},"sovereign":true}),
            json!({"providers":{"anthropic":{"enabled":false},"other":{}},"sovereign":"yes"}),
        ] {
            let mut old = environment.clone();
            if let Some(provider) = stored.get("defaultProvider") {
                old["defaultProvider"] = provider.clone();
            }
            for id in ["ollama", "openai", "anthropic"] {
                if let Some(provider) = stored["providers"].get(id) {
                    old["providers"][id] = provider.clone();
                }
            }
            old["sovereign"] = json!(stored["sovereign"] == true);
            let new = visible(environment.clone(), &stored).unwrap();
            assert_eq!(new.to_string(), old.to_string());
        }
    }
}
