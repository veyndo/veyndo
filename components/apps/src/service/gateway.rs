//! One permission-aware action gateway serves HTTP, UI and MCP; service egress is operator configured.
use crate::domain::mcp_views::{Mode, admit_tool};
use crate::domain::tool_output::checked_output;
use crate::domain::{action_admission, core_extensions};
use crate::ports::{HostCall, Platform};
use crate::service::service_call::service_call;
use crate::store::packages::package;
use crate::store::{action_log, jobs, managed_data};
use crate::types::events::AppDefined;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{channel, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad};

/// The host instructions a service action reply may carry, typed as they are read.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ServiceReply {
    #[serde(deserialize_with = "lenient")]
    purge_sources: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    export_cursor: Option<i64>,
}

/// Runs one app action for a caller and records its outcome without its payload.
pub async fn invoke(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    mcp: Option<Mode<'_>>,
) -> Result<Value> {
    let start = std::time::Instant::now();
    let result = execute(a, h, id, name, v, mcp).await;
    let status = result
        .as_ref()
        .map(|_| StatusCode::OK)
        .unwrap_or_else(|e| e.0);
    action_log::record(a, h, id, name, v, status, start.elapsed().as_millis()).await;
    result
}
async fn execute(
    a: &Platform,
    h: &RequestContext,
    id: &str,
    name: &str,
    v: &Value,
    mcp: Option<Mode<'_>>,
) -> Result<Value> {
    let host = a.actions.as_ref();
    let t = tenant(h)?;
    let m = package(a, &t, id, true).await?;
    let action = m
        .actions
        .iter()
        .find(|act| act.name == name)
        .ok_or(bad("Unknown app action"))?;
    if let Some(mode) = mcp {
        admit_tool(h, &m, action, &t, mode)?;
    }
    action_admission::check(h, action, v)?;
    if let Some(result) = host.hosted(&t, id, name).await? {
        return checked_output(action, result);
    }
    let e = action
        .entity
        .as_ref()
        .and_then(|n| m.entities.iter().find(|e| e.name == *n));
    if let Some(e) = e {
        core_extensions::access(e, h, action.handler == "save")?;
    }
    let result = match action.handler.as_str() {
        "list" => {
            managed_data::list_scoped(
                a,
                &t,
                &m,
                e.ok_or(bad("Entity required"))?,
                v,
                action.public.then(|| channel(h)),
            )
            .await
        }
        "save" => managed_data::save(a, &t, &m, e.ok_or(bad("Entity required"))?, v, h).await,
        "service" => {
            if !m.permissions.contains(&"service.call".into()) || m.runtime != "service" {
                return Err(Error(
                    StatusCode::FORBIDDEN,
                    "Service permission required".into(),
                ));
            }
            let result = service_call(a, &t, id, &format!("actions/{name}"), v).await?;
            let reply: ServiceReply = view(&result);
            if reply.purge_sources == Some(true)
                && m.permissions.contains(&"knowledge.write".into())
            {
                host.purge_sources(&t, id, reply.export_cursor.unwrap_or(0))
                    .await?;
            }
            Ok(result)
        }
        "job" => jobs::enqueue(a, h, &m, action, v).await,
        "emit" => {
            if v.to_string().len() > 16384 {
                return Err(bad("App event exceeds limit"));
            }
            let event = veyndo_core::events::emit_id_as(
                &a.db,
                &t,
                &format!("app.{id}.{name}"),
                &AppDefined(v.clone()),
            )
            .await?;
            Ok(json!({"eventId":event,"accepted":true}))
        }
        _ => {
            host.run(HostCall {
                actor: h,
                tenant: &t,
                manifest: &m,
                action,
                entity: e,
                input: v,
            })
            .await
        }
    }?;
    checked_output(action, result)
}
