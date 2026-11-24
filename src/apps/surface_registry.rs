//! The surface projection lives in the apps component; this module adds the languages of the shop.
use crate::commerce;
use crate::foundation::App;
use serde_json::Value;
use serde_json::json;
use veyndo_app_platform::store::surface_registry::Languages;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
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
