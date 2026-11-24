//! Product knowledge route; the intelligence component owns the facts and graph evidence.
use super::*;
pub(crate) async fn product_knowledge(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::product_knowledge(&Knowledge(&a), &h, &id).await?,
    ))
}
