//! Tenant-owned roles: current grants, revisioned edits, dependency-checked removal and shared history.
use crate::{App, SecureRoutes, commerce};
use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::routing::get;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) use veyndo_identity::service::roles::schema;
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route("/api/workspace/roles", &[("GET", "team.manage")], get(list))
        .secure_route(
            "/api/workspace/roles/{id}",
            &[("PUT", "team.manage"), ("DELETE", "team.manage")],
            axum::routing::put(save).delete(remove),
        )
        .secure_route(
            "/api/workspace/roles/{id}/dependencies",
            &[("GET", "team.manage")],
            get(dependencies),
        )
}
async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(invoke(&a, &h, "team.roles", &json!({})).await?))
}
async fn save(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    v["id"] = json!(id);
    Ok(Json(invoke(&a, &h, "team.role.save", &v).await?))
}
async fn remove(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    v["id"] = json!(id);
    Ok(Json(invoke(&a, &h, "team.role.delete", &v).await?))
}
async fn dependencies(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    Ok(Json(
        invoke(&a, &h, "team.role.dependencies", &json!({"id":id})).await?,
    ))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    veyndo_identity::service::roles::invoke(a, &Languages(a), h, name, v).await
}

/// The shop languages role names are written in.
struct Languages<'a>(&'a App);
impl veyndo_identity::ports::RoleLanguages for Languages<'_> {
    fn languages<'a>(
        &'a self,
        tenant: &'a str,
    ) -> veyndo_core::PortFuture<'a, (String, Vec<String>)> {
        Box::pin(async move {
            let (settings, _) = commerce::config(self.0, tenant).await?;
            Ok((settings.main_locale.clone(), settings.locales.clone()))
        })
    }
    fn check_names<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        names: &'a Value,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(async move {
            let (settings, _) = commerce::scoped_locked(tx, tenant, "default").await?;
            commerce::validate_names(names, &settings, 100)
        })
    }
}
