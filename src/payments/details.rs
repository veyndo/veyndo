//! Native payment-only history projection and optional owned provider observation; no financial command is executed.
use crate::{App, Json, Path, RequestContext, Result, State, Value, auth, merchant};

pub(crate) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(read(&a, &h, &id).await?))
}
pub(crate) async fn read(a: &App, h: &RequestContext, id: &str) -> Result<Value> {
    let tenant = merchant(a, h)?;
    auth::permit(h, "payments.read")?;
    veyndo_payments::service::details::read(&super::host::Gateway(a), a, &tenant, id).await
}
