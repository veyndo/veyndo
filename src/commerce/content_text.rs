//! Translated names are checked against the enabled content languages of the shop; the fallback rules are owned by the catalog component.
use super::*;
pub(crate) use veyndo_catalog::domain::content::{
    localize_extra, translated_object, translated_string,
};
pub(crate) fn validate_names(names: &Value, settings: &Settings, max: usize) -> Result<()> {
    let map = names
        .as_object()
        .filter(|m| !m.is_empty() && m.len() <= 100)
        .ok_or(bad("Translated names required"))?;
    for (key, value) in map {
        if !super::product_languages::allowed(key, settings)
            || !value.is_null() && value.as_str().is_none_or(|s| s.is_empty() || s.len() > max)
        {
            return Err(bad("Invalid translated name"));
        }
    }
    if names[&settings.main_locale]
        .as_str()
        .or_else(|| names[settings.main_locale.split('-').next().unwrap()].as_str())
        .is_none_or(|s| s.is_empty())
    {
        return Err(bad("Main language name required"));
    }
    Ok(())
}
