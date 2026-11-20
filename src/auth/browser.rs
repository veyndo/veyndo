//! Browser transport for the existing personal-session ledger: host-only HttpOnly cookies, exact origins and CSRF.
pub(crate) use veyndo_identity::http::browser::{apply, equal, origin};
