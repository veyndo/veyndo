//! Routes of the provider onboarding and listing; the payments component owns the account binding rules.
use super::host::{Gateway, PaymentHost};
use crate::{App, Json, Path, RequestContext, Result, State, Value};
use veyndo_payments::service::providers;

pub(crate) async fn onboarding(
    a: &App,
    h: &RequestContext,
    id: &str,
    input: &Value,
) -> Result<Value> {
    providers::onboarding((&PaymentHost, &Gateway(a)), a, h, id, input).await
}
pub(crate) async fn onboarding_route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(onboarding(&a, &h, &id, &v).await?))
}
pub(crate) async fn providers_route(
    State(a): State<App>,
    h: RequestContext,
) -> Result<Json<Value>> {
    Ok(Json(providers::providers(&a, &h).await?))
}
pub(crate) use providers::onboarding_action;
