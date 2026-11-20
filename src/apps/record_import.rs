//! Managed record import lives in the apps component; this module keeps the short path for the import operations.
use super::*;
use veyndo_app_platform::service::record_import;
pub(crate) async fn import_record(
    a: &App,
    h: &RequestContext,
    id: &str,
    destination: &Value,
    expected_digest: &str,
) -> Result<Value> {
    record_import::import_record(
        &service_limits::platform(a),
        h,
        id,
        destination,
        expected_digest,
    )
    .await
}
pub(crate) async fn inspect_import_record(
    a: &App,
    h: &RequestContext,
    id: &str,
    destination: &Value,
    expected_digest: &str,
) -> Result<(Value, Option<Value>)> {
    record_import::inspect_import_record(
        &service_limits::platform(a),
        h,
        id,
        destination,
        expected_digest,
    )
    .await
}
