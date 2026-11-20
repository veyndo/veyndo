//! Login/logout and personal workspace discovery are owned by the identity component.
pub(crate) use veyndo_identity::http::sessions::{user_login, user_logout, user_session};
pub(crate) use veyndo_identity::store::sessions::{
    SessionProof, issue_session, issue_session_with_proof, issue_verified_tx,
};
