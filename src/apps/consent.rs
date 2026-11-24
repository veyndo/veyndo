//! Package approval binds all requested permissions to a reviewed digest; current actor rights apply before every installation path.
use super::{Manifest, approval, service_limits, validate};
use crate::auth;
use crate::auth::SecureRoutes;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::post;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
/// The JSON fields `decode` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DecodeInput {
    #[serde(deserialize_with = "lenient")]
    built_in: Option<String>,
    manifest: Value,
}

pub(super) fn decode(v: &Value) -> Result<Manifest> {
    let input: DecodeInput = view(v);
    let m = if let Some(s) = input.built_in.as_deref().and_then(approval::built_in) {
        serde_json::from_str(s).map_err(|_| bad("Invalid built-in"))?
    } else {
        serde_json::from_value(input.manifest.clone()).map_err(|e| bad(e.to_string()))?
    };
    validate(&m)?;
    approval::installation(&m)?;
    Ok(m)
}
pub(crate) use veyndo_app_platform::domain::consent::{actor_permissions, approved};
pub(super) fn router() -> Router<App> {
    Router::new().secure_route("/api/apps/review", &[("POST", "apps.manage")], post(review))
}
/// The JSON fields `review` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewInput {
    #[serde(deserialize_with = "lenient")]
    environment: Option<String>,
}

async fn review(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: ReviewInput = view(&v);
    let t = merchant(&a, &h)?;
    let environment = input.environment.as_deref().filter(|e| !e.is_empty());
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
