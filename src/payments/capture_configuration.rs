//! Capture timing lives in the payments component; the server keeps this path for its callers.
pub(super) use veyndo_payments::domain::capture_intent::intent;
pub(crate) use veyndo_payments::store::capture_configuration::validate;
