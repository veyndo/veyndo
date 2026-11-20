//! Durable withdrawal/data-rights intake, tenant-scoped operator review and customer-held receipt access.
use super::*;
use veyndo_shop::store::consumer_requests::{self as store, Intake};
pub(super) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Intake>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::create(&a, &c, v).await?))
}
pub(super) async fn receipt(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let c = load_cart(&a, &h).await?;
    Ok(Json(store::receipt(&a, &c, &id).await?))
}
pub(super) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.read")?;
    Ok(Json(store::list(&a, &t).await?))
}
pub(super) async fn update(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "customers.write")?;
    Ok(Json(store::review(&a, &h, &t, &id, &v).await?))
}
