//! The package compiler adapter lives in the apps component; this module supplies the operator's publisher keys.
use super::*;
pub(crate) fn validate(m: &Manifest) -> Result<()> {
    veyndo_app_platform::domain::manifest_validation::validate(
        m,
        &crate::runtime_config::get().publishers,
    )
}
