//! Tenant-owned translated customer groups preserve exact rule IDs and select an explicit existing price/tax presentation basis.
use super::config;
use crate::auth;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn groups(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    if auth::permit(&h, "customers.read").is_err() {
        auth::permit(&h, "settings.read")?;
    }
    let (s, revision) = config(&a, &t).await?;
    Ok(Json(
        json!({"elements":s.customer_groups,"revision":revision,"mainLocale":s.main_locale,"availableLocales":s.locales}),
    ))
}
