//! Installed app tools share native wire metadata and validate declared results without disclosing provider payloads.
use crate::domain::action_admission::authorized;
use crate::domain::action_policy::readonly;
use crate::domain::approval;
use crate::domain::mcp_views::{available, location, uri};
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
    for m in active_manifests(a, h).await? {
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
