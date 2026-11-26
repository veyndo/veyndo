//! Flow configuration policy lives in the marketing component; the server lends it templates, apps and channel setup.
use super::configuration_host::Configuration;
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_marketing::service::configuration_writes as owner;
pub(crate) use veyndo_marketing::service::configuration_writes::{
    DRAFT_CAPABILITY, authorize_flow,
};

pub(super) async fn validate_draft(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    owner::validate_draft(&Configuration(&a), &h, &id, &v)
        .await
        .map(Json)
}

/// Native save and release validate final settings and installed app actions in their write transaction.
pub(crate) async fn validate_flow_config(
    a: &App,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    h: &RequestContext,
    value: &Value,
) -> Result<()> {
    owner::validate_flow_config(&Configuration(a), tx, t, h, value).await
}
