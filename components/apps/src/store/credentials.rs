//! App callback keys are identity integration keys; the identity store owns their SQL.
pub use veyndo_identity::store::app_credentials::{NewKey, active_count, issue, list, revoke};
