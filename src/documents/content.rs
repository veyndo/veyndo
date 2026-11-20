//! Source content is validated by the intelligence component against the shop's enabled languages.
use super::*;
pub(super) async fn validate(a: &App, t: &str, v: &Value) -> Result<Value> {
    let (settings, _) = commerce::config(a, t).await?;
    veyndo_intelligence::domain::source_content::validate(
        &settings.locales,
        &settings.main_locale,
        v,
    )
}
