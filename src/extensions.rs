//! Merchant catalogue route and the business policy routes; the apps component owns policy activation and state.
use crate::catalog_routes::catalog_request;
use crate::foundation::App;
use crate::foundation::merchant;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) async fn admin_catalog(
    State(a): State<App>,
    h: RequestContext,
    body: axum::body::Bytes,
) -> Result<Json<Value>> {
    merchant(&a, &h)?;
    catalog_request(State(a), h, body).await
}
pub(crate) async fn activate_extension(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_app_platform::service::business_policy::activate(&a, &a.apps.sandboxes, &h, &v)
            .await?,
    ))
}

pub(crate) async fn extension_state(
    State(a): State<App>,
    h: RequestContext,
) -> Result<Json<Value>> {
    Ok(Json(
        veyndo_app_platform::service::business_policy::state(&a, &h).await?,
    ))
}
