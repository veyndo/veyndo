//! The personal workspace lives in the studio component; the server registers its routes.
use crate::{App, SecureRoutes};
use axum::Router;
use axum::routing::get;
pub(crate) use veyndo_studio::domain::workspace::schema;
pub(crate) use veyndo_studio::http::workspace::invoke;
use veyndo_studio::http::workspace::{read, save, work};

pub(crate) fn router() -> Router<App> {
    Router::new()
        .secure_route("/api/studio/work", &[("GET", "read")], get(work))
        .secure_route(
            "/api/studio/workspace",
            &[("GET", "read"), ("PUT", "read")],
            get(read).put(save),
        )
}
