//! Product question route; the intelligence component owns the answer, its citations and the re-admission.
use super::*;
pub(crate) async fn question(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        product_questions::question(&Knowledge(&a), &h, &id, &v).await?,
    ))
}
