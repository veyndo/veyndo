//! Assistant app views live in the apps component; this module adds the caller's language and the gateway calls.
use super::*;
pub(super) use veyndo_app_platform::domain::mcp_views::Mode;
pub(crate) use veyndo_app_platform::domain::mcp_views::{
    EXTENSION, MIME, attachment_references, attachments,
};
pub(super) use veyndo_app_platform::domain::mcp_views::{admit_tool, document};
pub(crate) use veyndo_app_platform::store::mcp_views::list;

/// MCP opt-out applies to discovery and direct invocation, without disabling the UI/HTTP action.
pub(crate) async fn invoke_mcp(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
) -> Result<Value> {
    gateway::invoke(a, h, id, name, v, Some(Mode::tool())).await
}

pub(crate) async fn invoke_mcp_read(
    a: &App,
    h: &RequestContext,
    name: &str,
    v: &Value,
    resource: Option<&str>,
) -> Result<Value> {
    let (id, action) = name
        .strip_prefix("app.")
        .and_then(|name| name.split_once('.'))
        .ok_or(bad("Invalid app capability name"))?;
    gateway::invoke(a, h, id, action, v, Some(Mode::read(resource))).await
}

pub(crate) async fn read(a: &App, h: &RequestContext, requested: &str) -> Result<Option<Value>> {
    let Some((m, s)) = veyndo_app_platform::store::mcp_views::find(a, h, requested).await? else {
        return Ok(None);
    };
    let (locale, _) = language_context(a, h).await?;
    let (settings, _) = commerce::config(a, &tenant(h)?).await?;
    veyndo_app_platform::domain::mcp_views::resource(
        requested,
        &m,
        &s,
        &locale,
        &settings.main_locale,
    )
    .map(Some)
}

pub(super) async fn shell() -> Response {
    // Static sandbox shell. It has no tenant data or access to the caller's session.
    super::ui_documents::sandboxed(document(&json!({})))
}
