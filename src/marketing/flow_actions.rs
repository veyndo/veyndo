//! Native action schema and permissions use original Core names; no arbitrary SQL, shell or unguarded payment transitions.
use super::{execute_app_flow, flows};
use crate::foundation::App;
use crate::planner::plan_with;
use crate::{auth, commerce, operations};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, merge, view};
pub(crate) use veyndo_marketing::domain::flow_actions::{normalize, permission, validate};
/// The saved flow job an action runs from: its event, order and event context.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FlowDefinition {
    event: Value,
    #[serde(deserialize_with = "lenient")]
    order_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    event_context: EventDocument,
}

/// The document an event names.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EventDocument {
    #[serde(deserialize_with = "lenient")]
    document_id: Option<String>,
}

/// The saved configuration of one flow action.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ActionConfig {
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    app: Value,
    arguments: Value,
    instruction: Value,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    source_id: Value,
}

/// The arguments of a knowledge extraction a flow starts; the inference choice is sent only when the step has one.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtractArgs<'a, C: Serialize> {
    source_id: &'a str,
    product_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inference: Option<&'a C>,
}

/// The order fields a flow adds to the configured transition or receipt arguments.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderActionArgs<'a> {
    id: &'a str,
    revision: Value,
    request_key: &'a str,
    locale: &'a str,
}

/// A document's revision, passed on exactly as stored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Revisioned {
    revision: Value,
}

/// The result fields a flow reports after an order transition or receipt.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ActionResult {
    revision: Value,
    state: Value,
    id: Value,
}

pub(crate) async fn execute(
    a: &App,
    t: &str,
    f: &flows::Flow,
    key: &str,
    action: &str,
    config: &Value,
    definition: &Value,
) -> Result<Value> {
    let action_config: ActionConfig = view(config);
    let flow_job: FlowDefinition = view(definition);
    let normalized = normalize(action, config);
    let config = &normalized;
    validate(action, config)?;
    let h = super::flow_access::headers(a, t, f.actor.as_deref()).await?;
    auth::permit(&h, permission(action))?;
    if action == "knowledge.extract" {
        let source = action_config
            .source_id
            .as_str()
            .filter(|s| !s.is_empty())
            .or(flow_job.event_context.document_id.as_deref())
            .ok_or(bad("Extraction requires a document event or sourceId"))?;
        let args = json!(ExtractArgs {
            source_id: source,
            product_id: action_config
                .product_id
                .as_deref()
                .filter(|s| !s.is_empty()),
            inference: f.inference.as_ref(),
        });
        return crate::cognition::extraction::extract(a, &h, &args).await;
    }
    if action == "ai_proposal" {
        // The pipeline owner supplies this stable key; it is never model output.
        let (job, _) = key
            .strip_prefix("flow:")
            .and_then(|key| key.split_once(':'))
            .ok_or(bad("Flow job required for proposal"))?;
        let h = super::flow_access::proposal_headers(a, t, f.actor.as_deref(), job).await?;
        crate::performance::reserve_ai_attempt(a, t).await?;
        let (settings, _) = commerce::config(a, t).await?;
        let instruction = super::flow_text::effective(
            &action_config.instruction,
            &f.locale,
            &settings.main_locale,
        );
        return plan_with(
            a,
            t,
            &format!(
                "Flow event {}. {}. Create a reviewable proposal only.",
                flow_job.event, instruction
            ),
            f.inference.as_ref(),
            "",
            &f.locale,
            &h,
        )
        .await;
    }
    if action == "app_action" || action == "action.mail.send" {
        let mut step = f.clone();
        step.action = "app_action".into();
        step.app_action = Some(flows::AppFlowAction {
            app: action_config
                .app
                .as_str()
                .unwrap_or(if action == "action.mail.send" {
                    "email"
                } else {
                    ""
                })
                .into(),
            action: action_config
                .action
                .as_deref()
                .unwrap_or("send_order")
                .into(),
            arguments: if action == "action.mail.send" {
                json!({"locale":&f.locale[..2],"dryRun":false})
            } else {
                action_config.arguments.clone()
            },
        });
        return execute_app_flow(a, t, &step, key, "", definition).await;
    }
    let id = flow_job
        .order_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(bad("Action requires an order event"))?;
    if action == "action.set.order.state" || action == "action.generate.document" {
        let o: Revisioned = view(&veyndo_orders::store::order_reads::data(a, t, id).await?);
        let mut args = config.clone();
        merge(
            &mut args,
            OrderActionArgs {
                id,
                revision: o.revision,
                request_key: key,
                locale: &f.locale[..2],
            },
        )?;
        let value = operations::invoke(
            a,
            &h,
            if action == "action.set.order.state" {
                "merchant.order.transition"
            } else {
                "merchant.receipt.create"
            },
            &args,
        )
        .await?;
        let value: ActionResult = view(&value);
        return Ok(
            json!({"orderId":id,"revision":value.revision,"state":value.state,"documentId":if action=="action.generate.document" {value.id} else {Value::Null}}),
        );
    }
    super::flow_mutations::execute(a, t, f, key, action, config, id).await
}

#[cfg(test)]
mod typed_tests {
    use super::{ExtractArgs, OrderActionArgs};
    use serde_json::{Value, json};
    use veyndo_core::document::merge;

    #[test]
    fn extraction_arguments_keep_the_indexed_shape() {
        let choice = json!({"provider":"local","model":"m"});
        let mut old = json!({"sourceId":"doc-1","productId":Option::<&str>::None});
        old["inference"] = json!(&choice);
        let new = json!(ExtractArgs {
            source_id: "doc-1",
            product_id: None,
            inference: Some(&choice),
        });
        assert_eq!(new, old);
        let old = json!({"sourceId":"doc-1","productId":"p-1"});
        let new = json!(ExtractArgs::<Value> {
            source_id: "doc-1",
            product_id: Some("p-1"),
            inference: None,
        });
        assert_eq!(new, old);
    }

    #[test]
    fn order_action_arguments_keep_the_indexed_shape() {
        let config = json!({"state":"shipped","id":"ignored","note":null});
        let mut old = config.clone();
        old["id"] = json!("o-1");
        old["revision"] = json!(4);
        old["requestKey"] = json!("flow:1:2");
        old["locale"] = json!("de");
        let mut new = config;
        merge(
            &mut new,
            OrderActionArgs {
                id: "o-1",
                revision: json!(4),
                request_key: "flow:1:2",
                locale: "de",
            },
        )
        .unwrap();
        assert_eq!(new, old);
    }
}
