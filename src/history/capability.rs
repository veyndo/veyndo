//! HTTP/MCP parity for history; per-entity read/write scopes are checked again at invocation time.
use super::{routes, transport};
use crate::foundation::App;
use crate::{auth, commerce};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_studio::domain::history_access;
pub(crate) fn schema(name: &str) -> Option<Value> {
    if name == "merchant.customer.groups" {
        return Some(json!({"type":"object","properties":{},"additionalProperties":false}));
    }
    history_access::schema(name)
}
pub(crate) fn visible(h: &RequestContext, name: &str) -> bool {
    if name == "merchant.customer.groups" {
        return auth::permit(h, "customers.read").is_ok()
            || auth::permit(h, "settings.read").is_ok();
    }
    history_access::visible(h, name)
}
/// The record and version a history capability names.
#[derive(Deserialize, Default)]
#[serde(default)]
struct HistoryArgs {
    #[serde(deserialize_with = "lenient")]
    entity: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    before: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    version: Option<i64>,
}

pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let Json(value) = if name == "merchant.customer.groups" {
        commerce::groups(State(a.clone()), h.clone()).await?
    } else {
        let args: HistoryArgs = view(v);
        let entity = args.entity.ok_or(bad("entity required"))?;
        let id = args.id.ok_or(bad("id required"))?;
        match name {
            "merchant.history" => {
                transport::list(
                    State(a.core.clone()),
                    h.clone(),
                    Path((entity, id)),
                    axum::extract::Query(transport::Page {
                        before: args.before,
                    }),
                )
                .await?
            }
            "merchant.history.version" => {
                transport::detail(
                    State(a.core.clone()),
                    h.clone(),
                    Path((entity, id, args.version.ok_or(bad("version required"))?)),
                )
                .await?
            }
            "merchant.history.restore" => {
                routes::restore(
                    State(a.clone()),
                    h.clone(),
                    Path((entity, id, args.version.ok_or(bad("version required"))?)),
                    Json(v.clone()),
                )
                .await?
            }
            _ => return Err(bad("Unknown history operation")),
        }
    };
    Ok(value)
}
