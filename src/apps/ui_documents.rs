//! Sandboxed app documents are served by the apps component; this module registers their routes.
use super::*;
pub(crate) use veyndo_app_platform::http::ui_documents::SandboxedAppDocument;
pub(super) use veyndo_app_platform::http::ui_documents::sandboxed;
pub(super) fn router() -> Router<App> {
    Router::new()
        .route(
            "/app-ui/{id}/{digest}",
            get(veyndo_app_platform::http::ui_documents::document),
        )
        .route("/app-ui/mcp-view", get(super::mcp_views::shell))
}
