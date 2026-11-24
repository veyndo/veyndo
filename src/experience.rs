//! The storefront experience lives in the intelligence component; this handler supplies the experiment assignment secret.
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use std::env;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn experience(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let secret =
        env::var("PLATFORM_SECRET_KEY").unwrap_or_else(|_| a.identity.bootstrap_token.to_string());
    veyndo_intelligence::http::experience::experience(&a, &secret, h, v).await
}
