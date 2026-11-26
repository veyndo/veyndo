//! Country catalogue of a shop; the bundled world and the admission rules are owned by the shipping component.
use super::{Settings, config};
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
use veyndo_shipping::domain::geography;
pub(crate) use veyndo_shipping::domain::geography::Country;
pub(crate) fn catalogue(s: &Settings) -> Vec<Country> {
    geography::catalogue(&s.country_definitions)
}
pub(crate) async fn country_catalogue(
    State(a): State<App>,
    h: RequestContext,
) -> Result<Json<Value>> {
    let (s, revision) = config(&a, &tenant(&h)?).await?;
    Ok(Json(
        json!({"countries":catalogue(&s),"enabled":s.countries,"mainLocale":s.main_locale,"locales":s.locales,"revision":revision}),
    ))
}
