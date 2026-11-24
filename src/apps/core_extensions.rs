//! Namespaced core-object extension contracts live in the apps component; this module adds the native projections.
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;

pub(crate) use veyndo_app_model::Object;

/// Native lists and details share one bounded, permission-filtered app projection.
pub(crate) async fn attach(
    a: &crate::foundation::App,
    h: &RequestContext,
    object: Object,
    rows: &mut [serde_json::Value],
) -> Result<()> {
    veyndo_app_platform::store::extension_projection::attach(a, h, object, rows).await
}
