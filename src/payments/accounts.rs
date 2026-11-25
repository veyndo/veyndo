//! Routes of the provider onboarding and listing; the payments component owns the account binding rules.
use super::host::{Gateway, PaymentHost};
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
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
    Ok(Json(providers::providers(&Gateway(&a), &a, &h).await?))
}
pub(crate) use providers::onboarding_action;
