//! Product knowledge route; the intelligence component owns the facts and graph evidence.
use super::{Knowledge, product_questions};
use crate::{App, Json, Path, RequestContext, Result, State, Value};
pub(crate) async fn product_knowledge(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::product_knowledge(&Knowledge(&a), &h, &id).await?,
    ))
}
