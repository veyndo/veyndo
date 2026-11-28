//! Installed app tools share native wire metadata, carry their declared or conservative planner facts, and validate declared results without disclosing provider payloads.
use crate::domain::action_admission::authorized;
use crate::domain::action_policy::readonly;
use crate::domain::app_facts::Described;
use crate::domain::approval;
use crate::domain::mcp_views::{available, location, uri};
use crate::domain::package_contract::facts;
use crate::store::package_contracts;
use crate::store::packages::active_manifests;
use serde_json::{Value, json};
use veyndo_core::shop_scope::tenant;
use veyndo_core::tool_contract::ToolContract;
use veyndo_core::{RequestContext, Result, State};
use veyndo_kernel::verified_kernel;

/// The MCP metadata of a tool that opens an app view.
#[derive(serde::Serialize)]
struct ToolMeta {
    #[serde(rename = "_meta")]
    meta: Value,
}

/// The tools of the installed apps a caller may see, with the view each one opens.
pub async fn app_tools_for(a: &State, h: &RequestContext, public_only: bool) -> Result<Vec<Value>> {
    if h.principal.agent_key.is_some() {
        return Ok(Vec::new());
    }
    let mut tools = vec![];
    let contracts = package_contracts::active(&a.db, &tenant(h)?).await?;
    for m in active_manifests(a, h).await? {
        let contract = contracts
            .iter()
            .find(|(app, _)| app == &m.id)
            .map(|(_, c)| c);
        let digest = m
            .surfaces
            .iter()
            .any(|s| location(&s.location))
            .then(|| approval::canonical_digest(&m));
        for action in &m.actions {
            if (!public_only || action.public)
                && verified_kernel::app_tool_admissible(
                    action.mcp != Some(false),
                    authorized(h, action),
                )
            {
                let input_schema = m
                    .entities
                    .iter()
                    .find(|entity| {
                        action.handler == "list" && action.entity.as_deref() == Some(&entity.name)
                    })
                    .map(|entity| {
                        veyndo_app_model::query_contract::list_input_schema(
                            entity,
                            &action.input_schema,
                        )
                    })
                    .unwrap_or_else(|| action.input_schema.clone());
                let mut tool = ToolContract::describe(
                    &format!("app.{}.{}", m.id, action.name),
                    &action.description,
                    &input_schema,
                    action.output_schema.as_ref(),
                    readonly(action),
                );
                // The same facts, cost and approval members the native operation contracts carry.
                let (f, declared) = facts(&m, action, contract);
                veyndo_core::document::merge(&mut tool, Described::of(&f, declared))?;
                ToolContract::with_hints(&mut tool, f.hints());
                if let Some(digest) = &digest
                    && let Some(s) = m
                        .surfaces
                        .iter()
                        .find(|s| s.actions.first() == Some(&action.name) && available(h, &m, s))
                {
                    veyndo_core::document::merge(
                        &mut tool,
                        ToolMeta {
                            meta: json!({"ui":{"resourceUri":uri(&tenant(h)?, &m, s, digest)}}),
                        },
                    )?;
                }
                tools.push(tool);
            }
        }
    }
    Ok(tools)
}

/// The planner facts of one app tool (`app.<app>.<action>`) of an active package in the caller's
/// shop, and whether the publisher declared them; `None` when no such tool is installed. The
/// planner reads this; it is not an admission check, which stays with the gateway.
pub async fn tool_facts(
    a: &State,
    h: &RequestContext,
    name: &str,
) -> Result<Option<(crate::domain::app_facts::AppFacts, bool)>> {
    let Some((app, action)) = name.strip_prefix("app.").and_then(|n| n.split_once('.')) else {
        return Ok(None);
    };
    let t = tenant(h)?;
    let contract = package_contracts::of_app(&a.db, &t, app).await?;
    Ok(active_manifests(a, h)
        .await?
        .iter()
        .find(|m| m.id == app)
        .and_then(|m| {
            m.actions
                .iter()
                .find(|x| x.name == action)
                .map(|x| facts(m, x, contract.as_ref()))
        }))
}
