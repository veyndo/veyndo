//! Package approval binds all requested permissions to a reviewed digest; current actor rights apply before every installation path.
use super::{Manifest, approval, service_limits, validate};
use crate::{App, SecureRoutes, auth, merchant};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
pub(super) fn decode(v: &Value) -> Result<Manifest> {
    let m = if let Some(s) = v["builtIn"].as_str().and_then(approval::built_in) {
        serde_json::from_str(s).map_err(|_| bad("Invalid built-in"))?
    } else {
        serde_json::from_value(v["manifest"].clone()).map_err(|e| bad(e.to_string()))?
    };
    validate(&m)?;
    approval::installation(&m)?;
    Ok(m)
}
pub(crate) use veyndo_app_platform::domain::consent::{actor_permissions, approved};
pub(super) fn router() -> Router<App> {
    Router::new().secure_route("/api/apps/review", &[("POST", "apps.manage")], post(review))
}
async fn review(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    let environment = v["environment"].as_str().filter(|e| !e.is_empty());
    if environment.is_some() {
        auth::permit(&h, "developer.manage")?;
    }
    let m = decode(&v)?;
    actor_permissions(&h, &m)?;
    let (previous, upgrade) = veyndo_app_platform::service::upgrade_review::inspect(
        &service_limits::platform(&a),
        &t,
        environment,
        &m,
    )
    .await?;
    let old = previous.as_ref().map(|v| &v.permissions);
    let added = m
        .permissions
        .iter()
        .filter(|p| old.is_none_or(|v| !v.contains(p)))
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"app":m.id,"name":m.name,"version":m.version,"digest":approval::canonical_digest(&m),"permissions":m.permissions,"added":added,"previousVersion":previous.map(|v|v.version),"upgrade":upgrade}),
    ))
}
