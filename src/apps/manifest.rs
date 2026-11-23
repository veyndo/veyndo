//! Strict package contract; identifiers and limits are checked before logical model registration.
pub(crate) use veyndo_app_model::Entity;
pub(crate) use veyndo_app_model::manifest::Manifest;
pub(crate) use veyndo_app_model::surfaces::Surface;
pub(crate) fn identifier(s: &str) -> bool {
    veyndo_app_model::declarations::identifier(s)
}
