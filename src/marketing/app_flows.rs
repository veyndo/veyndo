//! App flow dispatch uses the same permission/schema gateway as HTTP/MCP, with a stable job key; admission lives in the app platform.
use super::flows;
use crate::foundation::App;
use crate::{apps, auth, payments};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::{Map, Value};
pub(crate) use veyndo_app_platform::service::flow_apps::validate_replacement;
use veyndo_app_platform::service::flow_apps::{arguments, eligible, require_instruction_grants};
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, merge, view};
/// The saved flow job an app action runs for.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FlowJob {
    order_id: Value,
    order_snapshot: Value,
    event_context: Value,
}

/// The property names an app action declares for its input.
#[derive(Deserialize, Default)]
#[serde(default)]
struct InputSchema {
    #[serde(deserialize_with = "lenient")]
    properties: Map<String, Value>,
}

/// The payment instructions added to the event an app action receives.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Instructions {
    payment_instructions: Value,
}

pub(crate) async fn execute_app_flow(
    a: &App,
    t: &str,
    f: &flows::Flow,
    id: &str,
    instruction: &str,
    definition: &Value,
) -> Result<Value> {
    let job: FlowJob = view(definition);
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
    let mut event = if job.order_id.as_str().is_some_and(|s| !s.is_empty()) {
        json!({"order":job.order_snapshot,"orderId":job.order_id})
    } else {
        job.event_context.clone()
    };
    if f.event == "payment.instructions.available"
        && view::<InputSchema>(&act.input_schema)
            .properties
            .contains_key("event")
    {
        require_instruction_grants(&m)?;
        let payment_instructions =
            payments::flow_instructions(a, &h, job.order_id.as_str().unwrap_or("")).await?;
        merge(
            &mut event,
            Instructions {
                payment_instructions,
            },
        )?;
    }
    let args = arguments(act, &target.arguments, (id, instruction), &event, &f.event);
    apps::invoke_app(a, &h, &target.app, &target.action, &args).await
}

#[cfg(test)]
mod tests {
    use super::Instructions;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn payment_instructions_keep_the_indexed_event_shape() {
        let instructions = json!({"iban":"DE00","reference":"R-1"});
        for event in [json!({"order":{"id":"o-1"},"orderId":"o-1"}), json!(null)] {
            let mut old = event.clone();
            old["paymentInstructions"] = instructions.clone();
            let mut new = event;
            merge(
                &mut new,
                Instructions {
                    payment_instructions: instructions.clone(),
                },
            )
            .unwrap();
            assert_eq!(new, old);
        }
    }
}
