//! The bounded input schema validator lives in the apps component; this module keeps the short path for the app adapters.
pub(crate) use veyndo_app_platform::domain::input_schema::{
    validate, validate_native, validate_value,
};
