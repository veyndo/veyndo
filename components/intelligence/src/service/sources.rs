//! Knowledge source content checked against the languages the shop has enabled.
use crate::domain::source_content::{self, Source};
use serde_json::Value;
use veyndo_core::{Result, State};
use veyndo_shop::service::read_cache::config;

/// Validates a source edit in the shop's languages.
pub async fn validate_content(a: &State, t: &str, v: &Value) -> Result<Source> {
    let (settings, _) = config(a, t).await?;
    source_content::validate(&settings.locales, &settings.main_locale, v)
}
