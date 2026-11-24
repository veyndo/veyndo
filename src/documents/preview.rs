//! Retrieval preview route; the intelligence component owns the audience and locale rules.
use super::*;
pub(crate) async fn preview(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::preview(&Knowledge(&a), &h, &v).await?,
    ))
}
