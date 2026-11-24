//! Operator-only platform inference administration: optimistic revision, encrypted write-only keys and an audit without secrets.
use crate::service::models::Models;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
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
    let row = sqlx::query(
        "SELECT revision,data,secrets,updated_at::text AS updated FROM platform_ai WHERE id=true",
    )
    .fetch_one(&a.db)
    .await?;
    let secrets: Value = row.get("secrets");
    let stored: Value = row.get("data");
    let mut visible = m.inference.environment_configuration();
    if let Some(provider) = stored.get("defaultProvider") {
        visible["defaultProvider"] = provider.clone();
    }
    for id in ["ollama", "openai", "anthropic"] {
        if let Some(provider) = stored["providers"].get(id) {
            visible["providers"][id] = provider.clone();
        }
    }
    visible["sovereign"] = json!(stored["sovereign"] == true);
    Ok(
        json!({"revision":row.get::<i64,_>("revision"),"settings":visible,
        "updatedAt":row.get::<String,_>("updated"),"encryptedStorageReady":settings::encryption_ready(),
        "keyStored":{"ollama":secrets.get("ollama").is_some(),"openai":secrets.get("openai").is_some(),"anthropic":secrets.get("anthropic").is_some()},
        "effective":m.inference.public_providers().await.ok(),"inheritance":"all tenants; replica refresh within 5 seconds"}),
    )
}
/// A platform inference configuration save.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlatformSettings {
    #[serde(deserialize_with = "lenient")]
    settings: SettingsInput,
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
                    || m.inference.environment_configuration()["providers"][id]["endpoint"].as_str()
                        == Some(s)
            })
            .ok_or(bad(
                "Provider endpoint must be HTTPS without credentials, query or fragment",
            ))?;
        let enabled = row.enabled.ok_or(bad("Provider enabled state required"))?;
        if id == provider && !enabled {
            return Err(bad("The default provider must be enabled"));
        }
        data["providers"][id] =
            json!({"model":model,"endpoint":endpoint.trim_end_matches('/'),"enabled":enabled});
    }
    let sovereign = input.settings.sovereign.unwrap_or(false);
    data["sovereign"] = json!(sovereign);
    // Refuse a switch that would only fail closed later: every configured model route must be self-operated.
    if sovereign || sovereign::locked() {
        let blockers = m.inference.sovereign_blockers(
            data["providers"]["ollama"]["endpoint"]
                .as_str()
                .unwrap_or_default(),
        );
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
    let row = sqlx::query("SELECT revision,secrets FROM platform_ai WHERE id=true FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if !verified_kernel::revision_admissible(
        v["revision"]
            .as_u64()
            .filter(|r| *r > 0)
            .ok_or(bad("Positive revision required"))?,
        row.get::<i64, _>("revision") as u64,
    ) {
        return Err(conflict("AI configuration changed; reload before saving"));
    }
    let mut secrets: Value = row.get("secrets");
    for id in ["ollama", "openai", "anthropic"] {
        if v["clearKeys"][id].as_bool() == Some(true) {
            secrets.as_object_mut().unwrap().remove(id);
        }
        if let Some(key) = v["keys"][id].as_str().filter(|s| !s.is_empty()) {
            if key.len() > 4096 || key.chars().any(char::is_control) {
                return Err(bad("Invalid provider key"));
            }
            secrets[id] = json!(settings::seal(id, key).map_err(bad)?);
        }
    }
    sqlx::query("UPDATE platform_ai SET revision=revision+1,data=$1,secrets=$2,updated_by=$3,updated_at=now() WHERE id=true").bind(data).bind(secrets).bind(actor).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO platform_audit(actor,action,data) VALUES($1,'ai.updated',$2)")
        .bind(actor)
        .bind(json!({"defaultProvider":provider,"sovereign":sovereign}))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    m.inference.invalidate().await;
    snapshot(m).await
}
