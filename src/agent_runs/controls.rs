//! Live-only agent stop and existing-key admission live in the intelligence component; the server registers the routes.
use crate::{App, Router, SecureRoutes, get};
use veyndo_intelligence::http::agents;
pub(crate) use veyndo_intelligence::service::agent_controls::{
    CAPABILITIES, admit_key, invoke, model_authority, schema, visible,
};
pub(crate) fn router() -> Router<App> {
    Router::new().secure_route(
        "/api/agent/control",
        &[("GET", "ai.manage"), ("POST", "ai.manage")],
        get(agents::control_status).post(agents::control),
    )
}
