//! Declared app settings reuse the digest-bound encrypted secret owner; secret values never have a browser read path.
use crate::domain::model_contract;
use crate::store::secrets;
use serde_json::{Value, json};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, Result, State, bad};

/// The declared settings of an app for its service; a removed declaration is never passed on.
pub async fn resolve(a: &State, t: &str, m: &Manifest) -> Result<Value> {
    let Some(encoded) = secrets::resolve(a, t, m, "settings").await? else {
        return Ok(json!({}));
    };
    let values: Value =
        serde_json::from_str(&encoded).map_err(|_| bad("Invalid encrypted app settings"))?;
    // A removed declaration never becomes available to a new package or remote service.
    Ok(veyndo_app_model::settings::declared(&m.settings, &values))
}

/// The native error of a shared settings failure.
pub fn settings_error(value: veyndo_app_storage::settings::Error<Error>) -> Error {
    match value {
        veyndo_app_storage::settings::Error::Storage(e) => secrets::error(e),
        veyndo_app_storage::settings::Error::Model(e) => model_contract::error(e),
    }
}
