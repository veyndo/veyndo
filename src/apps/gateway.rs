//! One permission-aware action gateway serves HTTP, UI and MCP; service egress is operator configured.
use super::*;
use veyndo_app_platform::ports::{ActionFuture, HostActions, HostCall, HostedFuture, RuleFuture};
pub(crate) fn validate_input(schema: &Value, v: &Value) -> Result<()> {
    input_schema::validate(schema, v)
}
pub(crate) fn validate_native_input(schema: &Value, v: &Value) -> Result<()> {
    input_schema::validate_native(schema, v)
}
/// The handlers the server owns, reached by the gateway after it admitted the call.
struct ServerActions<'a>(&'a App);
impl HostActions for ServerActions<'_> {
    fn hosted<'a>(&'a self, tenant: &'a str, app: &'a str, action: &'a str) -> HostedFuture<'a> {
        Box::pin(super::hosted::action(self.0, tenant, app, action))
    }
    fn run<'a>(&'a self, call: HostCall<'a>) -> ActionFuture<'a> {
        Box::pin(async move {
            let (a, h, v, id) = (self.0, call.actor, call.input, call.manifest.id.as_str());
            match call.action.handler.as_str() {
                "assets" | "asset_preview" => {
                    auth::permit(h, "catalog.read")?;
                    assets::app_file_callback(a, h, &call.action.handler, v).await
                }
                "asset_upload" => Err(bad(
                    "Asset uploads require the bounded multipart surface endpoint",
                )),
                "submit" => {
                    submissions::submit(
                        a,
                        h,
                        call.manifest,
                        call.entity.ok_or(bad("Entity required"))?,
                        v,
                    )
                    .await
                }
                "configure" => configure(a, h, id, v).await,
                "configurations" => {
                    merchant(a, h)?;
                    configurations(a, call.tenant, id, v).await
                }
                "payment_command" => crate::payments::app_command(a, h, id, v).await,
                "payment_onboarding" => crate::payments::onboarding(a, h, id, v).await,
                _ => Err(bad("Unsupported app action")),
            }
        })
    }
    fn purge_sources<'a>(&'a self, tenant: &'a str, app: &'a str, cursor: i64) -> RuleFuture<'a> {
        Box::pin(purge_sources(self.0, tenant, app, cursor))
    }
}
pub(crate) async fn invoke_app(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
) -> Result<Value> {
    invoke(a, h, id, name, v, None).await
}
pub(super) async fn invoke(
    a: &App,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    mcp: Option<mcp_views::Mode<'_>>,
) -> Result<Value> {
    veyndo_app_platform::service::gateway::invoke(
        &service_limits::platform(a),
        &ServerActions(a),
        h,
        id,
        name,
        v,
        mcp,
    )
    .await
}
pub(crate) async fn service_call(
    a: &App,
    t: &str,
    id: &str,
    path: &str,
    v: &Value,
) -> Result<Value> {
    veyndo_app_platform::service::service_call::service_call(
        &service_limits::platform(a),
        t,
        id,
        path,
        v,
    )
    .await
}

pub(crate) fn ui_url(id: &str) -> Option<String> {
    veyndo_app_platform::domain::service_ui::ui_url(id, &crate::runtime_config::get().services)
}

/// The registry uses the same live permission check as direct actions.
pub(super) fn action_authorized(_a: &App, h: &RequestContext, action: &Action) -> bool {
    veyndo_app_platform::domain::action_admission::authorized(h, action)
}
pub(crate) use veyndo_app_platform::domain::action_admission::additional as additional_permissions;
