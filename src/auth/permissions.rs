//! Fine-grained workspace overrides. Owners retain control; delegates cannot grant rights they lack.
pub(crate) use veyndo_identity::http::access::{access, revoke_invite, revoke_session, sessions};
pub(crate) use veyndo_identity::{SCOPES, allowed, validate_permissions};
