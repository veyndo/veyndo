//! Declared app settings reuse the digest-bound encrypted secret owner; secret values never have a browser read path.
use crate::domain::approval;
use crate::store::packages::package;
use crate::store::secrets::Crypto;
use crate::store::settings::settings_error;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::parent;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

/// The declared settings of an app; secret values are never returned.
pub async fn read(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "team.manage")?;
    if !verified_kernel::persistent_key_admissible(
        h.principal.personal_session,
        permit(&h, "team.manage").is_ok(),
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal administrator session required".into(),
        ));
    }
    let m = package(&a, &t, &id, false).await?;
    let mut tx = a.db.begin().await?;
    let mut value = veyndo_app_storage::settings::read(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: &t,
            app: &id,
        },
        &m,
        &Crypto,
    )
    .await
    .map_err(settings_error)?;
    tx.commit().await?;
    value["canManage"] = json!(parent(&a, &t).await?.is_none());
    value["encryptionReady"] = json!(veyndo_cognition::inference::settings::encryption_ready());
    Ok(Json(value))
}
/// An app settings save for the approved package digest at its revision.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SettingsSave {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    digest: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    values: Value,
}

/// Saves declared settings under the approved package digest.
pub async fn save(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: SettingsSave = view(&v);
    let t = merchant(&h)?;
    permit(&h, "team.manage")?;
    if !verified_kernel::persistent_key_admissible(
        h.principal.personal_session,
        permit(&h, "team.manage").is_ok(),
    ) || parent(&a, &t).await?.is_some()
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Settings require a personal live-shop administrator session".into(),
        ));
    }
    let m = package(&a, &t, &id, true).await?;
    let digest = approval::canonical_digest(&m);
    if input.approve != Some(true) || input.digest.as_deref() != Some(digest.as_str()) {
        return Err(conflict(
            "Approve the current package before saving settings",
        ));
    }
    let revision = input
        .revision
        .filter(|n| *n >= 0)
        .ok_or(bad("Settings revision required"))?;
    let mut tx = a.db.begin().await?;
    let revision = veyndo_app_storage::settings::save(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: &t,
            app: &id,
        },
        &m,
        &veyndo_app_storage::settings::Update {
            digest: &digest,
            revision,
            actor: h
                .principal
                .user
                .as_deref()
                .ok_or(bad("Personal account required"))?,
        },
        &input.values,
        &Crypto,
    )
    .await
    .map_err(settings_error)?;
    tx.commit().await?;
    Ok(Json(json!({"saved":true,"revision":revision})))
}
