//! Native lifecycles live in the apps component; this module keeps the short path for orders, quotes and settings.
pub(crate) use veyndo_app_platform::domain::native_lifecycle::{
    allowed, order_machine, project_labels, reason, snapshot, validate_locale,
};
pub(crate) use veyndo_app_platform::store::native_lifecycle::{
    bind_order, capture, current_order, validate_active_locale, validate_order_base,
};
