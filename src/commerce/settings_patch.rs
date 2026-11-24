//! Sparse settings overrides are owned by the shop component.
use super::Settings;
use crate::{Result, Value, marketing};
pub(crate) fn resolve(base: &Settings, patch: Value) -> Result<Settings> {
    veyndo_shop::domain::patch::resolve(base, patch, &marketing::validate_condition)
}
