//! Native action schema and permissions use original Core names; no arbitrary SQL, shell or unguarded payment transitions.
use super::*;
pub(crate) use veyndo_marketing::domain::flow_actions::{normalize, permission, validate};
pub(crate) async fn execute(
    a: &App,
    t: &str,
    f: &flows::Flow,
    key: &str,
    action: &str,
    config: &Value,
    definition: &Value,
) -> Result<Value> {
    let normalized = normalize(action, config);
    let config = &normalized;
    validate(action, config)?;
    let h = super::flow_access::headers(a, t, f.actor.as_deref()).await?;
    auth::permit(&h, permission(action))?;
    if action == "knowledge.extract" {
        let source = config["sourceId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| definition["eventContext"]["documentId"].as_str())
            .ok_or(bad("Extraction requires a document event or sourceId"))?;
        let mut args = json!({"sourceId":source,"productId":config["productId"].as_str().filter(|s| !s.is_empty())});
        if let Some(choice) = &f.inference {
            args["inference"] = json!(choice);
        }
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
        let instruction =
            super::flow_text::effective(&config["instruction"], &f.locale, &settings.main_locale);
        return plan_with(
            a,
            t,
            &format!(
                "Flow event {}. {}. Create a reviewable proposal only.",
                definition["event"], instruction
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
            app: config["app"]
                .as_str()
                .unwrap_or(if action == "action.mail.send" {
                    "email"
                } else {
                    ""
                })
                .into(),
            action: config["action"].as_str().unwrap_or("send_order").into(),
            arguments: if action == "action.mail.send" {
                json!({"locale":&f.locale[..2],"dryRun":false})
            } else {
                config["arguments"].clone()
            },
        });
        return execute_app_flow(a, t, &step, key, "", definition).await;
    }
    let id = definition["orderId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(bad("Action requires an order event"))?;
    if action == "action.set.order.state" || action == "action.generate.document" {
        let o = veyndo_orders::store::order_reads::data(a, t, id).await?;
        let mut args = config.clone();
        args["id"] = json!(id);
        args["revision"] = o["revision"].clone();
        args["requestKey"] = json!(key);
        args["locale"] = json!(&f.locale[..2]);
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
        return Ok(
            json!({"orderId":id,"revision":value["revision"],"state":value["state"],"documentId":if action=="action.generate.document" {value["id"].clone()} else {Value::Null}}),
        );
    }
    super::flow_mutations::execute(a, t, f, key, action, config, id).await
}
