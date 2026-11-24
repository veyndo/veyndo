//! Authoritative deletion guards for methods; the reference counts are owned by the shipping component.
use crate::{App, Json, Path, RequestContext, Result, State, Value, json};
pub(crate) async fn method_dependencies(
    State(a): State<App>,
    h: RequestContext,
    path: Path<(String, String)>,
) -> Result<Json<Value>> {
    let Json(found) =
        veyndo_shipping::http::method_usage::dependencies(State(a.core.clone()), h, path).await?;
    Ok(Json(json!(found)))
}
