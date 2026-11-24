//! Imported promotions are saved by the marketing component's configuration owner.
use super::configuration_host::Configuration;
use crate::{App, RequestContext, Result, Value};
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
