//! Entry points for routes, capabilities and the worker; image jobs are owned by the catalog component.
use crate::App;
use axum::Json;
use axum::extract::FromRef;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_catalog::http::image_jobs as owner;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(super) async fn provider(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    owner::provider(State(FromRef::from_ref(&a)), h).await
}
pub(super) async fn enqueue(
    State(a): State<App>,
    h: RequestContext,
    product: Path<String>,
    v: Json<Value>,
) -> Result<Json<Value>> {
    owner::enqueue(State(FromRef::from_ref(&a)), h, product, v).await
}
pub(super) async fn list_jobs(
    State(a): State<App>,
    h: RequestContext,
    product: Path<String>,
) -> Result<Json<Value>> {
    owner::list(State(FromRef::from_ref(&a)), h, product).await
}
pub(super) async fn detail(
    State(a): State<App>,
    h: RequestContext,
    id: Path<String>,
) -> Result<Json<Value>> {
    owner::detail(State(FromRef::from_ref(&a)), h, id).await
}
pub(super) async fn apply(
    State(a): State<App>,
    h: RequestContext,
    id: Path<String>,
    v: Json<Value>,
) -> Result<Json<Value>> {
    owner::apply(State(FromRef::from_ref(&a)), h, id, v).await
}
pub(crate) async fn image_once(a: &App) -> Result<bool> {
    veyndo_catalog::service::image_jobs::once(&FromRef::from_ref(a)).await
}
