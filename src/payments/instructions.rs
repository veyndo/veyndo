//! Customer-capability and compound merchant access to typed recipient instructions; reads never settle an invoice.
use super::customer;
use crate::{App, Json, Path, RequestContext, Result, State, Value, auth, merchant, tenant};
pub(crate) async fn customer_read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    customer(a, h, id).await?;
    read(a, &tenant(h)?, id).await
}
pub(crate) async fn merchant_read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    let t = merchant(a, h)?;
    for scope in ["payments.read", "orders.read", "customers.pii"] {
        auth::permit(h, scope)?;
    }
    read(a, &t, id).await
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
    Ok(Json(merchant_read(&a, &h, &id).await?))
}
async fn read(a: &App, t: &str, id: &str) -> Result<Value> {
    veyndo_payments::service::instructions::read(&super::host::Gateway(a), a, t, id).await
}
