//! Personal MFA enrollment and one-use sign-in challenges share the native session and encryption owners.
use crate::auth::SecureRoutes;
use crate::foundation::App;
use axum::Router;
use axum::routing::get;
use axum::routing::post;
pub(crate) use veyndo_identity::http::mfa::personal;
use veyndo_identity::http::mfa::{confirm, disable, enroll, status, verify};
pub(super) use veyndo_identity::store::sessions::{challenge_tx, trusted_session_tx};
pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route("/api/auth/mfa", &[("GET", "read")], get(status))
        .secure_route("/api/auth/mfa/enroll", &[("POST", "read")], post(enroll))
        .secure_route("/api/auth/mfa/confirm", &[("POST", "read")], post(confirm))
        .secure_route("/api/auth/mfa/disable", &[("POST", "read")], post(disable))
        .secure_route("/api/auth/mfa/verify", &[("POST", "public")], post(verify))
}
