//! Live-only agent stop and existing-key admission; no control becomes a sandbox release or credential snapshot.
use crate::store::agent_controls as store;
use crate::types::events::AgentsControlled;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_app_model::declarations::identifier;
use veyndo_cognition::inference::budget::Authority;
use veyndo_core::document::{lenient, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{allowed, merchant};
use veyndo_kernel::verified_kernel;

/// The conversation owner of a caller: the agent key, else the user.
pub fn conversation_actor(h: &RequestContext) -> Option<String> {
    h.principal
        .agent_key
        .as_ref()
        .map(|id| format!("agent:{id}"))
        .or_else(|| h.principal.user.clone())
}
/// The agent control operations.
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "merchant.agents.status",
        "Read live shop agent stop state and revision",
    ),
    (
        "merchant.agents.control",
        "Explicitly stop or re-enable shop agents with current revision",
    ),
];
/// The input schema of an agent control operation.
pub fn schema(name: &str) -> Option<Value> {
    CAPABILITIES.iter().any(|(n,_)|*n==name).then(|| if name.ends_with("status") {
        json!({"type":"object","properties":{},"additionalProperties":false})
    } else { json!({"type":"object","properties":{"stopped":{"type":"boolean"},"revision":{"type":"integer","minimum":1},"approve":{"type":"boolean","enum":[true]}},"required":["stopped","revision","approve"],"additionalProperties":false}) })
}
/// Whether the caller may see and change the shop's agent controls.
pub fn visible(h: &RequestContext) -> bool {
    h.principal.personal_session && merchant(h).is_ok() && allowed(h, "ai.manage")
}
/// The budget authority a run executes under.
pub fn model_authority(h: &RequestContext) -> Authority {
    let tenant = h
        .access
        .as_ref()
        .and_then(|s| s.parent.clone())
        .or_else(|| h.principal.tenant.clone())
        .or_else(|| {
            h.access
                .as_ref()
                .filter(|s| s.exists)
                .map(|s| s.tenant.clone())
        })
        .unwrap_or_default();
    Authority {
        tenant,
        key_id: h.principal.agent_key.clone(),
        require_budget: h.principal.agent_key.is_some()
            || (!h.principal.personal_session
                && h.principal
                    .user
                    .as_deref()
                    .is_some_and(|s| s != "bootstrap")
                && h.principal.app.is_none()
                && header(h, "authorization").is_some()),
    }
}
/// Admits a request made with a budgeted agent key: planning only, and only while the key and shop agents are live.
pub async fn admit_key(a: &State, h: &RequestContext, path: &str, method: &str) -> Result<()> {
    if let Some(id) = &h.principal.agent_key {
        let run_control = path
            .strip_prefix("/api/agent/runs/")
            .and_then(|s| s.rsplit_once('/'))
            .is_some_and(|(id, action)| identifier(id) && ["cancel", "resume"].contains(&action));
        let planning_post = method == "POST"
            && (run_control
                || [
                    "/mcp",
                    "/api/agent/runs",
                    "/api/agent/chat",
                    "/api/agent/chat/stream",
                    "/api/agent/plan",
                    "/api/merchant/grid/plan",
                ]
                .contains(&path));
        if !verified_kernel::planning_request_admissible(
            ["GET", "HEAD"].contains(&method),
            planning_post,
        ) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Budgeted planning agents cannot mutate commerce or delegate credentials".into(),
            ));
        }
        let permitted = store::key_permitted(&a.db, id, &tenant(h)?).await?;
        if !permitted {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Agent access is stopped or revoked".into(),
            ));
        }
    }
    Ok(())
}
/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    stopped: Option<bool>,
}

/// Reads or changes the live shop's agent stop state.
pub async fn invoke(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: InvokeInput = view(v);
    validate_native(&schema(name).ok_or(bad("Unknown agent control"))?, v)?;
    if !visible(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Personal AI administration permission required".into(),
        ));
    }
    if name.ends_with("control") && input.approve != Some(true) {
        return Err(bad("Explicit approval required for agent controls"));
    }
    let t = merchant(h)?;
    let mut tx = a.db.begin_privileged().await?;
    let live = store::live_shop(&mut *tx, &t).await?;
    let row = store::switch_locked(&mut *tx, &live).await?;
    let revision = row.agents_revision;
    let stopped = row.agents_stopped;
    if name.ends_with("status") {
        return Ok(json!({"stopped":stopped,"revision":revision,"liveShop":live}));
    }
    if input.revision != Some(revision) {
        return Err(conflict("Agent controls changed; reload before confirming"));
    }
    let stopped = input.stopped.unwrap();
    store::set_stopped(&mut *tx, &live, stopped).await?;
    let cancelled = if stopped {
        store::cancel_all(&mut tx, &live).await?
    } else {
        0
    };
    let controlled = AgentsControlled {
        stopped,
        revision: revision + 1,
        actor: h.principal.user.as_deref(),
        cancelled,
    };
    veyndo_core::events::emit(&mut *tx, &live, &controlled).await?;
    tx.commit().await?;
    Ok(json!({"stopped":stopped,"revision":revision+1,"liveShop":live,"cancelled":cancelled}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_have_closed_schemas() {
        assert!(schema("merchant.agents.status").is_some());
        assert!(
            schema("merchant.agents.control").unwrap()["required"]
                .as_array()
                .unwrap()
                .len()
                == 3
        );
        assert!(schema("merchant.agents.other").is_none());
    }
    #[test]
    fn anonymous_callers_have_no_actor_and_no_budget() {
        let h = RequestContext::default();
        assert_eq!(conversation_actor(&h), None);
        assert!(!visible(&h));
        assert!(!model_authority(&h).require_budget);
    }
}
