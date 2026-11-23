//! Durable order-event flows: conditions, shop notes and AI proposals; no unapproved model mutations.
use super::*;
pub(crate) use veyndo_marketing::store::flow_projection::project as project_flows;
pub(crate) use veyndo_marketing::types::flow::{AppFlowAction, Flow};
pub(crate) async fn flow_once(a: &App) -> Result<bool> {
    let Some(job) = super::flow_jobs::claim(a).await? else {
        return Ok(false);
    };
    let (id, t, definition) = (job.id.as_str(), job.tenant.as_str(), &job.definition);
    let f: Flow =
        serde_json::from_value(definition["flow"].clone()).map_err(|_| bad("Invalid flow"))?;
    let (settings, _) = commerce::config(a, t).await?;
    let instruction =
        super::flow_text::effective(&json!(f.instruction), &f.locale, &settings.main_locale)
            .to_string();
    let authorized = super::flow_access::headers(a, t, f.actor.as_deref())
        .await
        .is_ok();
    let result: Result<Value> = if !authorized {
        Err(Error(
            StatusCode::FORBIDDEN,
            "Flow owner lost access".into(),
        ))
    } else if f.action == "pipeline" {
        if job.execution["finishAfterDelay"] == true {
            Ok(json!({"completed":true}))
        } else {
            match super::pipeline_runtime::run(a, &job, &f).await {
                Ok(super::pipeline_runtime::Outcome::Finished(value)) => Ok(value),
                Ok(super::pipeline_runtime::Outcome::Deferred) => return Ok(true),
                Err(error) => Err(error),
            }
        }
    } else if f.action == "work_proposal" {
        match f.work_template.as_ref() {
            Some(template) => {
                veyndo_studio::service::flow_templates::prepare(
                    &crate::agent_host::Host(a),
                    (t, id),
                    &f,
                    template,
                    definition,
                )
                .await
            }
            None => Err(bad("Work template required")),
        }
    } else if f.action == "app_action" {
        execute_app_flow(a, t, &f, id, &instruction, definition).await
    } else if f.action == "note" {
        Ok(json!({"note":instruction,"orderId":definition["orderId"],"locale":f.locale}))
    } else if f.action == "ai_proposal" {
        let context = format!(
            "Flow event {}. {}. Create a reviewable proposal only.",
            definition["event"], instruction
        );
        match super::flow_access::proposal_headers(a, t, f.actor.as_deref(), id).await {
            Ok(h) => match crate::performance::reserve_ai_attempt(a, t).await {
                Ok(()) => plan_with(a, t, &context, f.inference.as_ref(), "", &f.locale, &h).await,
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    } else {
        Err(bad("Unsupported stored Flow action"))
    };
    let (state, value, error) = match result {
        Ok(v) => ("completed", Some(v), None),
        Err(e) => ("failed", None, Some(e.1)),
    };
    let mut tx = a.db.begin().await?;
    if job.finish(&mut tx, state, &value, error).await?
        && state == "completed"
        && f.action == "note"
        && !definition["orderId"].as_str().unwrap_or("").is_empty()
    {
        sqlx::query("INSERT INTO order_activity(tenant,order_id,actor,kind,data) VALUES($1,$2,$3,'flow',$4)").bind(t).bind(definition["orderId"].as_str().unwrap_or("")).bind(f.actor.as_deref().unwrap_or("flow")).bind(json!({"text":instruction,"flowJob":id,"event":definition["event"],"locale":f.locale})).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(true)
}
