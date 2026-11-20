//! Revision-bound source detail, editing, archive/restore and publication; edits require a new public review.
use super::*;
use veyndo_intelligence::store::documents as store;
pub(crate) async fn detail(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.read")?;
    Ok(Json(store::detail(&a, &t, &id).await?))
}
pub(crate) async fn edit(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    guidance::admit(&h, &v["kind"])?;
    let data = content::validate(&a, &t, &v).await?;
    Ok(Json(store::edit(&a, &h, &t, &id, &v, &data).await?))
}
pub(crate) async fn lifecycle(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    Ok(Json(store::lifecycle(&a, &h, &t, &id, &v).await?))
}
