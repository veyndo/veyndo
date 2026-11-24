//! App flow dispatch uses the same permission/schema gateway as HTTP/MCP, with a stable job key; admission lives in the app platform.
use super::flows;
use crate::{App, apps, auth, payments};
use serde_json::Value;
use serde_json::json;
pub(crate) use veyndo_app_platform::service::flow_apps::validate_replacement;
use veyndo_app_platform::service::flow_apps::{arguments, eligible, require_instruction_grants};
use veyndo_core::Result;
use veyndo_core::bad;
pub(crate) async fn execute_app_flow(
    a: &App,
    t: &str,
    f: &flows::Flow,
    id: &str,
    instruction: &str,
    definition: &Value,
) -> Result<Value> {
    let target = f
        .app_action
        .as_ref()
        .ok_or(bad("Flow app action required"))?;
    let mut h = super::flow_access::headers(a, t, f.actor.as_deref()).await?;
    h.insert(
        "idempotency-key",
        format!("flow-{id}")
            .parse()
            .map_err(|_| bad("Invalid flow job identity"))?,
    );
    auth::permit(&h, "automation.manage")?;
    auth::permit(&h, "apps.manage")?;
    let m = apps::package(a, t, &target.app, true).await?;
    let act = m
        .actions
        .iter()
        .find(|x| x.name == target.action)
        .ok_or(bad("Unknown flow app action"))?;
    eligible(act)?;
    let mut event = if definition["orderId"]
        .as_str()
        .is_some_and(|s| !s.is_empty())
    {
        json!({"order":definition["orderSnapshot"],"orderId":definition["orderId"]})
    } else {
        definition["eventContext"].clone()
    };
    if f.event == "payment.instructions.available"
        && act.input_schema["properties"].get("event").is_some()
    {
        require_instruction_grants(&m)?;
        event["paymentInstructions"] =
            payments::flow_instructions(a, &h, definition["orderId"].as_str().unwrap_or(""))
                .await?;
    }
    let args = arguments(act, &target.arguments, (id, instruction), &event, &f.event);
    apps::invoke_app(a, &h, &target.app, &target.action, &args).await
}
