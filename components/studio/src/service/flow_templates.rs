//! Flow work templates: bounded copies of reviewed native work whose event values replace explicit scalar fields, never executable text.
use super::work_plan::bind;
use crate::domain::proposal::Step;
use crate::ports::WorkHost;
use crate::store::tasks;
use serde_json::{Value, json};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, hash, uid};
use veyndo_identity::allowed;
use veyndo_intelligence::service::run_journal::persist;
use veyndo_marketing::domain::flow_text::effective;
use veyndo_marketing::store::flow_access::proposal_headers;
use veyndo_marketing::types::flow::Flow;
use veyndo_marketing::types::work_template::WorkTemplate;
use veyndo_shop::service::read_cache::config;

/// Binds event values and stable run references into the reviewed steps; only explicit scalar fields change, never executable text.
pub fn resolve(
    template: &WorkTemplate,
    mut steps: Vec<Step>,
    job: &str,
    definition: &Value,
) -> Result<Vec<Step>> {
    template.validate()?;
    for binding in &template.bindings {
        let args: Value = serde_json::from_str(
            &steps
                .get(binding.step)
                .ok_or(bad("Template step does not exist"))?
                .arguments_json,
        )
        .map_err(|_| bad("Invalid template arguments"))?;
        if args
            .pointer(&binding.pointer)
            .is_none_or(|v| !v.is_string() && !v.is_number())
        {
            return Err(bad("Template field must be existing text or a number"));
        }
    }
    for (index, step) in steps.iter_mut().enumerate() {
        let mut args: Value = serde_json::from_str(&step.arguments_json)
            .map_err(|_| bad("Invalid template arguments"))?;
        for binding in template.bindings.iter().filter(|b| b.step == index) {
            let value = if let Some(key) = binding.source.strip_prefix("run.key:") {
                json!(format!("work_{}", &hash(&format!("{job}:{key}"))[..24]))
            } else {
                let key = binding.source.strip_prefix("event.").unwrap();
                let value = definition["eventContext"][key].clone();
                if key == "revision" {
                    if value.as_i64().is_none_or(|v| v < 0) {
                        return Err(bad("Event has no valid revision for this template"));
                    }
                } else if value.as_str().is_none_or(|v| v.is_empty() || v.len() > 160) {
                    return Err(bad(format!("Event has no {key} for this template")));
                }
                value
            };
            let target = args
                .pointer_mut(&binding.pointer)
                .ok_or(bad("Template target disappeared"))?;
            if target.is_string() != value.is_string() || target.is_number() != value.is_number() {
                return Err(bad("Event value type does not match template input"));
            }
            *target = value;
        }
        step.arguments_json = args.to_string();
    }
    Ok(steps)
}

/// The completed, applied work of the actor in this private workspace, with its reviewed steps; bindings must target existing scalar fields.
pub async fn load(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    h: &RequestContext,
    template: &WorkTemplate,
) -> Result<Vec<Step>> {
    template.validate()?;
    let actor = header(h, "x-rac-user").ok_or(bad("Work template actor required"))?;
    if !h.principal.personal_session
        && header(h, "x-rac-flow-job").is_none()
        && actor != "bootstrap"
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal task access required".into(),
        ));
    }
    let (evidence, scopes) = tasks::completed_work(tx, t, &template.task_id, actor)
        .await?
        .ok_or(Error(StatusCode::FORBIDDEN, "Choose your completed work in this private workspace; source tasks cannot be published or copied across workspaces".into()))?;
    if scopes.as_array().is_none_or(|values| {
        values
            .iter()
            .any(|v| v.as_str().is_none_or(|scope| !allowed(h, scope)))
    }) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Source task requires its original permissions".into(),
        ));
    }
    let mut steps: Vec<Step> = serde_json::from_value(evidence["proposal"]["operations"].clone())
        .map_err(|_| bad("Source task has no native work"))?;
    if steps.is_empty() || steps.len() > 12 {
        return Err(bad("Source task needs 1..12 reviewed steps"));
    }
    // Completed evidence is immutable. Bindings are execution grants and are
    // deliberately rebuilt by the native owner before the new review.
    for step in &mut steps {
        step.binding = None;
    }
    // Validate targets without resolving absent future event values.
    for binding in &template.bindings {
        let args: Value = serde_json::from_str(
            &steps
                .get(binding.step)
                .ok_or(bad("Unknown template step"))?
                .arguments_json,
        )
        .map_err(|_| bad("Invalid source work arguments"))?;
        if args.pointer(&binding.pointer).is_none_or(|v| {
            if binding.source == "event.revision" {
                !v.is_number()
            } else {
                !v.is_string()
            }
        }) {
            return Err(bad(
                "Map an existing text or number field in the source task",
            ));
        }
    }
    Ok(steps)
}

/// Copies the template's reviewed work for one flow job into a new task that still needs approval.
pub async fn prepare<H: WorkHost>(
    host: &H,
    (t, job): (&str, &str),
    f: &Flow,
    template: &WorkTemplate,
    definition: &Value,
) -> Result<Value> {
    let a = host.models().core;
    let h = proposal_headers(a, t, f.actor.as_deref(), job).await?;
    let steps = {
        let mut tx = a.db.begin().await?;
        load(&mut tx, t, &h, template).await?
    };
    let mut steps = resolve(template, steps, job, definition)?;
    // Native owners validate current tenant, schema, package, rights and original
    // revisions. A copy never refreshes an obsolete revision behind the review.
    bind(host, t, &h, &mut steps).await?;
    let (settings, _) = config(a, t).await?;
    let instruction = effective(&json!(f.instruction), &f.locale, &settings.main_locale).to_owned();
    let id = uid();
    let evidence = json!({"instruction":instruction,"locale":f.locale,"actor":f.actor,"model":null,"inference":null,"catalogBefore":[],"applied":false,"flowTemplateLimit":template.daily_limit,"proposal":{"summary":instruction,"changes":[],"operations":steps},"templateSource":{"taskId":template.task_id,"flowJob":job,"bindings":template.bindings}});
    persist(host, t, &id, &evidence, &h).await?;
    Ok(json!({"taskId":id,"preview":evidence,"approvalRequired":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn template(bindings: Value) -> WorkTemplate {
        serde_json::from_value(json!({"taskId":"12abc", "dailyLimit":2,"bindings":bindings}))
            .unwrap()
    }
    fn steps() -> Vec<Step> {
        vec![serde_json::from_value(json!({"name":"merchant.product.save","label":"Edit","arguments_json":"{\"id\":\"original\",\"parent\":\"original\",\"revision\":3,\"text\":\"{event.productId}\"}"})).unwrap()]
    }
    #[test]
    fn event_mapping_is_typed_and_never_interpolates_unselected_text() {
        let t = template(
            json!([{"step":0,"pointer":"/id","source":"event.productId"},{"step":0,"pointer":"/revision","source":"event.revision"}]),
        );
        let out = resolve(
            &t,
            steps(),
            "job",
            &json!({"eventContext":{"productId":"target","revision":5}}),
        )
        .unwrap();
        let args: Value = serde_json::from_str(&out[0].arguments_json).unwrap();
        assert_eq!(
            args,
            json!({"id":"target","parent":"original","revision":5,"text":"{event.productId}"})
        );
        assert!(
            resolve(
                &t,
                steps(),
                "job",
                &json!({"eventContext":{"productId":"target","revision":"5"}})
            )
            .is_err()
        );
        assert!(resolve(&t, steps(), "job", &json!({})).is_err());
    }
    #[test]
    fn generated_references_are_stable_within_one_event_and_distinct_between_events() {
        let t = template(
            json!([{"step":0,"pointer":"/id","source":"run.key:product"},{"step":0,"pointer":"/parent","source":"run.key:product"}]),
        );
        let a = resolve(&t, steps(), "one", &json!({})).unwrap();
        let b = resolve(&t, steps(), "one", &json!({})).unwrap();
        let c = resolve(&t, steps(), "two", &json!({})).unwrap();
        assert_eq!(a[0].arguments_json, b[0].arguments_json);
        assert_ne!(a[0].arguments_json, c[0].arguments_json);
        let args: Value = serde_json::from_str(&a[0].arguments_json).unwrap();
        assert_eq!(args["id"], args["parent"]);
    }
    #[test]
    fn malformed_duplicate_and_unknown_mappings_fail_closed() {
        for bindings in [
            json!([{"step":0,"pointer":"/id","source":"event.cookie"}]),
            json!([{"step":12,"pointer":"/id","source":"event.productId"}]),
            json!([{"step":0,"pointer":"/id","source":"event.productId"},{"step":0,"pointer":"/id","source":"run.key:other"}]),
        ] {
            assert!(template(bindings).validate().is_err());
        }
        let t = template(json!([{"step":0,"pointer":"/absent","source":"event.productId"}]));
        assert!(
            resolve(
                &t,
                steps(),
                "job",
                &json!({"eventContext":{"productId":"target"}})
            )
            .is_err()
        );
        let mut t = template(json!([]));
        t.daily_limit = 0;
        assert!(t.validate().is_err());
    }
}
