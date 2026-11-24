//! Manual order route; the studio component owns the draft, review and approved completion behind the register port.
use super::*;
pub(super) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Path(operation): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, &operation, &v).await?))
}
pub(crate) async fn invoke(
    a: &App,
    h: &RequestContext,
    operation: &str,
    v: &Value,
) -> Result<Value> {
    Box::pin(veyndo_studio::service::manual_orders::invoke(
        &crate::pos::Register::new(a),
        a,
        h,
        operation,
        v,
    ))
    .await
}
