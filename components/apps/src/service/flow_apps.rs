//! App steps of flows use the same permission and schema gateway as HTTP and MCP: admission, installed-package checks, upgrade safety and argument binding.
//! Action input schemas and step arguments are app-declared open JSON; native schema validation types them.
use crate::domain::action_inputs::InputProperties;
use crate::domain::{action_admission, core_extensions, field_values};
use crate::store::{flows, locks, packages};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::model_contract::validate as validate_input;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_identity::permit;
use veyndo_kernel::verified_kernel;
use veyndo_marketing::domain::flow_steps::app_steps;
use veyndo_marketing::store::flow_access::headers_on;
use veyndo_marketing::types::flow::Flow;

/// Rejects actions the verified kernel does not admit for unattended flows.
pub fn eligible(action: &Action) -> Result<()> {
    if !verified_kernel::app_flow_admissible(action.flow_allowed, action.read_only, action.public) {
        return Err(bad("Action is not eligible for flows"));
    }
    Ok(())
}

/// The actor may publish this app step: app, action and entity rights, explicit payment/PII grants and a valid contract.
pub fn validate_step(h: &RequestContext, f: &Flow, m: &Manifest) -> Result<()> {
    permit(h, "apps.manage")?;
    let target = f
        .app_action
        .as_ref()
        .ok_or(bad("Flow app action required"))?;
    let act = m
        .actions
        .iter()
        .find(|x| x.name == target.action)
        .ok_or(bad("Unknown flow app action"))?;
    eligible(act)?;
    if f.event == "payment.instructions.available"
        && InputProperties::of(&act.input_schema).declares("event")
    {
        for scope in ["payments.read", "orders.read", "customers.pii"] {
            permit(h, scope)?;
        }
        require_instruction_grants(m)?;
    }
    permit(h, act.permission.as_deref().unwrap_or("catalog"))?;
    action_admission::additional(h, act)?;
    if let Some(entity) = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == act.entity.as_ref())
    {
        core_extensions::access(entity, h, act.handler == "save")?;
    }
    validate_contract(f, m)
}

/// Payment recipient instructions reach an app only with its explicit payment and PII permissions.
pub fn require_instruction_grants(m: &Manifest) -> Result<()> {
    if !["payments.instructions.read", "customers.pii"]
        .iter()
        .all(|scope| m.permissions.iter().any(|p| p == scope))
    {
        return Err(bad(
            "App needs explicit payment and PII permission for recipient instructions",
        ));
    }
    Ok(())
}

fn validate_contract(f: &Flow, m: &Manifest) -> Result<()> {
    let target = f
        .app_action
        .as_ref()
        .ok_or(bad("Flow app action required"))?;
    let act = m
        .actions
        .iter()
        .find(|a| a.name == target.action)
        .ok_or(conflict(
            "Upgrade would remove an action used by an active flow",
        ))?;
    eligible(act)?;
    let event = if ["order.", "delivery.", "document.", "payment.", "pos."]
        .iter()
        .any(|prefix| f.event.starts_with(prefix))
    {
        json!({"orderId":"00000000000000000000000000000000"})
    } else {
        json!({})
    };
    let args = arguments(
        act,
        &target.arguments,
        ("schema-check", "schema-check"),
        &event,
        &f.event,
    );
    validate_input(&act.input_schema, &args)?;
    if act.handler == "save" {
        let entity = m
            .entities
            .iter()
            .find(|e| Some(&e.name) == act.entity.as_ref())
            .ok_or(bad("Flow app model missing"))?;
        field_values::validate(entity, &view::<SaveArguments>(&args).fields)?;
    }
    Ok(())
}

/// Every app step of a flow names an installed, active package whose action the actor may publish; `lock` fences concurrent installs.
pub async fn validate_installed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    h: &RequestContext,
    f: &Flow,
    lock: bool,
) -> Result<()> {
    for step in app_steps(f) {
        let target = step
            .app_action
            .as_ref()
            .ok_or(bad("Flow app action required"))?;
        if lock {
            locks::advisory(tx, &format!("{t}:{}", target.app), 7).await?;
        }
        let row = packages::stored_package(tx, t, &target.app)
            .await?
            .ok_or(bad("Flow app is not installed"))?;
        if !row.active {
            return Err(conflict("Flow app is inactive"));
        }
        let m: Manifest =
            serde_json::from_value(row.manifest).map_err(|_| bad("Invalid installed manifest"))?;
        validate_step(h, &step, &m)?;
    }
    Ok(())
}

/// Called under the existing automation lock, on every installation transport and release.
pub async fn validate_replacement(
    conn: &mut sqlx::PgConnection,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    let rows = flows::active(conn, t).await?;
    let invalid = || {
        conflict("Upgrade would invalidate an active flow; update its action and arguments first")
    };
    for row in rows {
        let f: Flow = serde_json::from_value(row).map_err(|_| bad("Invalid stored flow"))?;
        for step in app_steps(&f) {
            if step.app_action.as_ref().is_some_and(|a| a.app == m.id) {
                let h = headers_on(&mut *conn, t, f.actor.as_deref())
                    .await
                    .map_err(|_| invalid())?;
                validate_step(&h, &step, m).map_err(|_| invalid())?;
            }
        }
    }
    Ok(())
}

/// The JSON fields `arguments` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ArgumentsInput {
    #[serde(deserialize_with = "lenient")]
    order_id: Option<String>,
}

/// The record a save step writes, as its arguments carry it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SaveArguments {
    fields: Value,
}

/// The step arguments `arguments` reads before binding.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StepArguments {
    #[serde(deserialize_with = "lenient")]
    order_id: Option<String>,
}

/// The inputs the host binds into step arguments; unset ones keep the merchant's value.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct BoundArguments<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    order_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<&'a str>,
}

/// Binds the declared inputs of an app action: the canonical order ID of order-bound events, kind, request key, event and template.
pub fn arguments(
    action: &Action,
    base: &Value,
    (key, instruction): (&str, &str),
    event: &Value,
    kind: &str,
) -> Value {
    let input: ArgumentsInput = view(event);
    let mut args = base.clone();
    let props = InputProperties::of(&action.input_schema);
    let mut bound = BoundArguments::default();
    // Native order-bound events supply their canonical ID; explicit non-empty
    // merchant arguments remain unchanged. No string interpolation or AI identity.
    if props.declares("orderId")
        && view::<StepArguments>(&args)
            .order_id
            .as_deref()
            .is_none_or(str::is_empty)
    {
        bound.order_id = input.order_id.as_deref().filter(|id| !id.is_empty());
    }
    if props.declares("kind") {
        bound.kind = Some(kind);
    }
    if props.declares("requestKey") {
        bound.request_key = Some(key);
    }
    if props.declares("event") {
        bound.event = Some(event);
    }
    if props.declares("template") && args.get("template").is_none() {
        bound.template = Some(instruction);
    }
    let binds = bound.order_id.is_some()
        || bound.kind.is_some()
        || bound.request_key.is_some()
        || bound.event.is_some()
        || bound.template.is_some();
    // Arguments that are not an object stay as they are; input validation rejects them afterwards.
    if binds && merge(&mut args, bound).is_err() {
        return base.clone();
    }
    args
}
