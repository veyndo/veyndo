//! Hosted frontend routes and the server side of the managed-app install; the apps component owns binding and disconnection.
use crate::{App, apps, auth};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::tenant;
pub(super) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    auth::permit(&h, "settings.read")?;
    Ok(Json(
        json!({"frontends":apps::hosted::connections(&a,&tenant(&h)?).await?}),
    ))
}
pub(super) async fn bind(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_app_platform::service::frontend_bindings::bind(&Install, &a, &h, &v).await?,
    ))
}
pub(super) async fn remove(
    State(a): State<App>,
    h: RequestContext,
    Path(alias): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_app_platform::service::frontend_bindings::remove(&a, &h, &alias, &v).await?,
    ))
}

/// The bundled managed apps of this process.
struct Install;
impl veyndo_app_platform::ports::HostedInstall for Install {
    fn ensure<'a, 'c: 'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        tenant: &'a str,
        id: &'a str,
    ) -> veyndo_core::PortFuture<'a, ()> {
        Box::pin(apps::hosted::ensure(tx, tenant, id))
    }
}
