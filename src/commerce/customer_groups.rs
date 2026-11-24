//! Tenant-owned translated customer groups preserve exact rule IDs and select an explicit existing price/tax presentation basis.
use super::config;
use crate::{App, Json, RequestContext, Result, State, Value, auth, json, merchant};
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
