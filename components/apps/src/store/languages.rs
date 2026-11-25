//! App record translations use configured shop languages and field-level main-language inheritance.
use crate::domain::model_contract;
use serde_json::Value;
use veyndo_app_model::Entity;
use veyndo_core::Result;

/// Checks the translated fields of a record against the languages the shop enabled.
pub async fn validate_languages(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    e: &Entity,
    fields: &Value,
) -> Result<()> {
    let data: Value = veyndo_shop::store::settings::basis(tx, t).await?;
    let settings = veyndo_shop::domain::defaults::decode(data)?;
    veyndo_app_model::language::validate(e, fields, &settings.locales, &settings.main_locale)
        .map_err(model_contract::error)
}
