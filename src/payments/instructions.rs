//! Routes of typed recipient instructions; the payments component owns the customer and compound merchant checks.
use crate::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_payments::service::instructions;
pub(crate) async fn customer_read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    instructions::customer_read(&super::host::Gateway(a), a, h, id).await
}
pub(crate) async fn customer_route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(customer_read(&a, &h, &id).await?))
}
pub(crate) async fn merchant_route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        instructions::merchant_read(&super::host::Gateway(&a), &a, &h, &id).await?,
    ))
}
