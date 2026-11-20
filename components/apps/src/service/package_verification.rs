//! A package proves its executable parts before it is stored: frozen configuration examples and the commerce hook component.
use crate::domain::{field_values, model_contract};
use crate::service::configuration_examples;
use crate::service::configuration_runtime::{prepare_source, validate_fee};
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Result, bad};

/// Verifies the executable parts of a package: its configuration examples and its commerce hook component.
pub async fn verify_package(m: &Manifest) -> Result<()> {
    if let Some(c) = &m.configuration {
        let entity = m
            .entities
            .iter()
            .find(|e| e.name == c.entity)
            .ok_or(bad("Configuration model missing"))?;
        field_values::validate(entity, &c.default_fields)?;
        validate_fee(
            c,
            c.default_fields[&c.price_field]
                .as_i64()
                .ok_or(bad("Default price required"))?,
        )
        .await?;
        configuration_examples::verify(c).await?;
    }
    let Some(c) = &m.commerce_hooks else {
        return Ok(());
    };
    let guest = prepare_source(c.source.clone()).await?;
    let manifest = m.clone();
    tokio::task::spawn_blocking(move || {
        guest.validate_component_abi().map_err(bad)?;
        veyndo_apps::business_examples::verify(manifest.capability.as_ref(), &guest)
            .map_err(model_contract::error)
    })
    .await
    .map_err(|_| bad("Component verification worker failed"))?
}
