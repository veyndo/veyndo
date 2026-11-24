//! Translated names are checked against the enabled content languages of the shop; the fallback rules are owned by the catalog component.
use super::Settings;
use serde_json::Value;
pub(crate) use veyndo_catalog::domain::content::translated_string;
use veyndo_core::Result;
pub(crate) fn validate_names(names: &Value, settings: &Settings, max: usize) -> Result<()> {
    settings.languages().validate_names(names, max)
}
