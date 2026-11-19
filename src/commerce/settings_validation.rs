//! Configuration validation is owned by the shop component; rule conditions are checked by the rule engine.
use super::*;

pub(crate) fn validate_config(s: &Settings) -> Result<()> {
    veyndo_shop::domain::validation::validate(s, &marketing::validate_condition)
}
