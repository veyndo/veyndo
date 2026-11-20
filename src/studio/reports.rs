//! Shared native commerce reports for Studio, workspace, API and agent reads; no second analytics ledger.
use crate::*;
pub(crate) mod config;
pub(crate) use config::schema;
pub(crate) fn visible(h: &RequestContext) -> bool {
    auth::allowed(h, "overview.read") || auth::allowed(h, "orders.read")
}
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route("/api/merchant/reports", &[("POST", "read")], post(http))
}
async fn http(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    if !visible(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Shop reporting permission required".into(),
        ));
    }
    let (settings, _) = commerce::config(a, &t).await?;
    let (locale, chain) = language_context(a, h).await?;
    veyndo_studio::store::reports::run(
        a,
        h,
        &t,
        v,
        veyndo_studio::store::reports::Context {
            settings: &settings,
            locale: &locale,
            chain: &chain,
            eligible: include_str!("../cognition/behavior/eligible.sql"),
            conditions: &marketing::validate_condition,
        },
    )
    .await
}
