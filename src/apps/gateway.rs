//! One permission-aware action gateway serves HTTP, UI and MCP; service egress is operator configured.
use super::{
    configurations, configure, input_schema, mcp_views, purge_sources, service_limits, submissions,
};
use crate::{App, assets, auth, language_context, marketing, merchant, staging};
use serde_json::Value;
use veyndo_app_platform::ports::{
    ActionFuture, EnvironmentFuture, HostActions, HostCall, HostedFuture, RuleFuture,
};
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
pub(crate) fn validate_native_input(schema: &Value, v: &Value) -> Result<()> {
    input_schema::validate_native(schema, v)
}
/// The handlers the server owns, reached by the gateway after it admitted the call.
pub(super) struct ServerActions(pub(super) App);
impl HostActions for ServerActions {
    fn hosted<'a>(&'a self, tenant: &'a str, app: &'a str, action: &'a str) -> HostedFuture<'a> {
        Box::pin(super::hosted::action(&self.0, tenant, app, action))
    }
    fn run<'a>(&'a self, call: HostCall<'a>) -> ActionFuture<'a> {
        Box::pin(async move {
            let (a, h, v, id) = (&self.0, call.actor, call.input, call.manifest.id.as_str());
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
    fn visitor_object<'a>(
        &'a self,
        h: &'a RequestContext,
        key: &'a str,
        id: &'a str,
    ) -> RuleFuture<'a> {
        Box::pin(async move {
            let a = &self.0;
            match key {
                "customerId" | "unitId" => crate::b2b::surface_context(a, h, key, id).await?,
                "productId" => marketing::admit_product(a, h, id).await?,
                _ => {
                    if id != marketing::channel_id(h) {
                        return Err(bad("Surface channel differs from the current shop context"));
                    }
                    marketing::channel(a, h, id, &language_context(a, h).await?.0).await?;
                }
            }
            Ok(())
        })
    }
    fn purge_sources<'a>(&'a self, tenant: &'a str, app: &'a str, cursor: i64) -> RuleFuture<'a> {
        Box::pin(purge_sources(&self.0, tenant, app, cursor))
    }
    fn create_preview<'a>(
        &'a self,
        h: &'a RequestContext,
        actor: &'a str,
    ) -> EnvironmentFuture<'a> {
        Box::pin(async move {
            let created = staging::create_preview(&self.0, h, "App Studio", actor).await?;
            created["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or(bad("Preview environment unavailable"))
        })
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
    veyndo_app_platform::service::gateway::invoke(&service_limits::platform(a), h, id, name, v, mcp)
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

pub(crate) use veyndo_app_platform::domain::action_admission::additional as additional_permissions;
