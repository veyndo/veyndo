//! Native lifecycles live in the apps component; this module keeps the short path for orders, quotes and settings.
pub(crate) use veyndo_app_platform::store::native_lifecycle::{
    bind_order, capture, validate_active_locale,
};
