//! Private cloned shops live in the studio component; the server registers their routes and lends the release port.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use axum::routing::post;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
mod release_host;
pub(crate) use release_host::Releases;
pub(crate) use veyndo_core::shop_scope::live_parent as parent_conn;
pub(crate) use veyndo_studio::service::staging::snapshot;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route(
            "/api/environments",
            &[
                ("GET", "environments.read"),
                ("POST", "environments.manage"),
            ],
            get(veyndo_studio::service::staging::list).post(create),
        )
        .secure_route(
            "/api/environments/{id}/diff",
            &[("GET", "environments.read")],
            get(veyndo_studio::service::staging::diff),
        )
        .secure_route(
            "/api/environments/{id}/release",
            &[("POST", "environments.manage")],
            post(release),
        )
}
pub(crate) async fn live(a: &App, h: &RequestContext) -> Result<String> {
    veyndo_studio::service::staging::live(a, h).await
}
pub(crate) async fn owned(a: &App, live: &str, stage: &str) -> Result<Value> {
    veyndo_studio::service::staging::owned(a, live, stage).await
}
async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_studio::service::staging::create(&Releases(&a), h, v).await?,
    ))
}
pub(crate) async fn create_preview(
    a: &App,
    h: &RequestContext,
    name: &str,
    actor: &str,
) -> Result<Value> {
    veyndo_studio::service::staging::create_preview(&Releases(a), h.clone(), name, actor).await
}
async fn release(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_studio::service::staging::release(&Releases(&a), h, id, v).await?,
    ))
}
