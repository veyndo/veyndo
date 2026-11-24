//! Imported promotions are saved by the marketing component's configuration owner.
use super::configuration_host::Configuration;
use crate::foundation::App;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn import_promotion(
    a: &App,
    h: &RequestContext,
    id: &str,
    data: &Value,
) -> Result<Value> {
    veyndo_marketing::service::configuration_writes::import_promotion(
        &Configuration(a),
        h,
        id,
        data,
    )
    .await
}
