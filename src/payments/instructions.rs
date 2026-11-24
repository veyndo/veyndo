//! Routes of typed recipient instructions; the payments component owns the customer and compound merchant checks.
use crate::{App, Json, Path, RequestContext, Result, State, Value};
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
