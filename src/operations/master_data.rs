//! HTTP entry points of the company basis; storage and validation are owned by the orders component.
use super::*;
use veyndo_orders::store::company;
pub(crate) use veyndo_orders::store::company::{lock, public_source, validate_release};
pub(super) async fn read(
    a: &App,
    h: &RequestContext,
    channel: Option<&str>,
) -> Result<Json<Value>> {
    let t = merchant(a, h)?;
    Ok(Json(company::read(a, h, &t, channel).await?))
}
pub(super) async fn get(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "settings.read")?;
    read(&a, &h, None).await
}
pub(super) async fn get_channel(
    State(a): State<App>,
    h: RequestContext,
    Path(channel): Path<String>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "settings.read")?;
    read(&a, &h, Some(&channel)).await
}
pub(super) async fn put(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "settings.write")?;
    save(&a, &h, v, None).await
}
pub(super) async fn put_channel(
    State(a): State<App>,
    h: RequestContext,
    Path(channel): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    auth::permit(&h, "settings.write")?;
    save(&a, &h, v, Some(&channel)).await
}
pub(crate) async fn save(
    a: &App,
    h: &RequestContext,
    v: Value,
    channel: Option<&str>,
) -> Result<Json<Value>> {
    let t = merchant(a, h)?;
    Ok(Json(company::save(a, h, &t, v, channel).await?))
}
