//! Route of the native payment-only history projection; no financial command is executed.
use crate::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(read(&a, &h, &id).await?))
}
pub(crate) async fn read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    veyndo_payments::service::merchant_operations::details(&super::host::Gateway(a), a, h, id).await
}
