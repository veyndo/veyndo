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
use veyndo_app_platform::domain::package_contract::{self, Contract};
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
    contract: Value,
}

/// The Manifest and its signed package contract: the bundled one for a built-in app, else the
/// request's `contract`. A package without one keeps conservative tool defaults and no ports.
pub(super) fn decode_package(v: &Value) -> Result<(Manifest, Option<Contract>)> {
    let input: DecodeInput = view(v);
    let built_in = input.built_in.as_deref();
    let m: Manifest = if let Some(s) = built_in.and_then(approval::built_in) {
        serde_json::from_str(s).map_err(|_| bad("Invalid built-in"))?
    } else {
        serde_json::from_value(input.manifest.clone()).map_err(|e| bad(e.to_string()))?
    };
    validate(&m)?;
    approval::installation(&m)?;
    let contract: Option<Contract> = match built_in.and_then(package_contract::built_in) {
        Some(s) => Some(serde_json::from_str(s).map_err(|_| bad("Invalid built-in contract"))?),
        None if input.contract.is_null() => None,
        None => Some(
            serde_json::from_value(input.contract.clone())
                .map_err(|e| bad(format!("Invalid package contract: {e}")))?,
        ),
    };
    if let Some(c) = &contract {
        package_contract::validate(&m, c, &crate::runtime_config::get().publishers)?;
    }
    Ok((m, contract))
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
    let (m, contract) = decode_package(&v)?;
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
        json!({"app":m.id,"name":m.name,"version":m.version,"digest":approval::canonical_digest(&m),"contractDigest":contract.as_ref().map(package_contract::digest),"contract":contract,"permissions":m.permissions,"added":added,"previousVersion":previous.map(|v|v.version),"upgrade":upgrade}),
    ))
}
