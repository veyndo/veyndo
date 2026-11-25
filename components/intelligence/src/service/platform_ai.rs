//! Operator-only platform inference administration: optimistic revision, encrypted write-only keys and an audit without secrets.
use crate::service::models::Models;
use crate::store::platform_ai as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_cognition::inference::{settings, sovereign};
use veyndo_core::document::{lenient, view};
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
    let mut visible = m.inference.environment_configuration();
    if let Some(provider) = stored.get("defaultProvider") {
        visible["defaultProvider"] = provider.clone();
    }
    for id in ["ollama", "openai", "anthropic"] {
        if let Some(provider) = view::<Configuration>(stored).providers.get(id) {
            visible["providers"][id] = provider.clone();
        }
    }
    visible["sovereign"] = json!(view::<Configuration>(stored).sovereign == true);
    Ok(json!({"revision":row.revision,"settings":visible,
        "updatedAt":row.updated,"encryptedStorageReady":settings::encryption_ready(),
        "keyStored":{"ollama":secrets.get("ollama").is_some(),"openai":secrets.get("openai").is_some(),"anthropic":secrets.get("anthropic").is_some()},
        "effective":m.inference.public_providers().await.ok(),"inheritance":"all tenants; replica refresh within 5 seconds"}))
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
    let mut data = json!({"defaultProvider":provider,"providers":{}});
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
        data["providers"][id] = json!({"model":model,"endpoint":endpoint,"enabled":enabled});
    }
    let sovereign = input.settings.sovereign.unwrap_or(false);
    data["sovereign"] = json!(sovereign);
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
            secrets[id] = json!(settings::seal(id, key).map_err(bad)?);
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
