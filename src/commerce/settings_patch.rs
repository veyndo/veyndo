//! Sparse settings overrides are owned by the shop component.
use super::Settings;
use crate::marketing;
use serde_json::Value;
use veyndo_core::Result;
pub(crate) fn resolve(base: &Settings, patch: Value) -> Result<Settings> {
    veyndo_shop::domain::patch::resolve(base, patch, &marketing::validate_condition)
}
