//! The surface projection lives in the apps component; this module adds the languages of the shop.
use super::*;
use veyndo_app_platform::store::surface_registry::Languages;
pub(crate) async fn registry(a: &App, h: &RequestContext, public: bool) -> Result<Value> {
    let (settings, _) = commerce::config(a, &tenant(h)?).await?;
    veyndo_app_platform::store::surface_registry::registry(
        a,
        h,
        public,
        Languages {
            main: &settings.main_locale,
            enabled: &json!(settings.locales),
        },
    )
    .await
}
