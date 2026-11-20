//! Namespaced core-object extension contracts live in the apps component; this module adds the native projections.
use crate::{RequestContext, Result};

pub(crate) use veyndo_app_model::Object;
pub(crate) use veyndo_app_platform::domain::core_extensions::{access, configuration};

pub(crate) async fn descriptions(
    a: &crate::App,
    h: &RequestContext,
    object: Object,
) -> Result<serde_json::Value> {
    super::extension_projection::descriptions(a, h, object).await
}

/// Native lists and details share one bounded, permission-filtered app projection.
pub(crate) async fn attach(
    a: &crate::App,
    h: &RequestContext,
    object: Object,
    rows: &mut [serde_json::Value],
) -> Result<()> {
    super::extension_projection::attach(a, h, object, rows).await
}
