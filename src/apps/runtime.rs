//! The configuration executor lives in the apps component; this module keeps the short path for cart and data adapters.
pub(crate) use veyndo_app_platform::service::configuration_runtime::{
    configured_contribution, prepare, prepare_source, validate_fee,
};
