//! HTTP/MCP parity for history; per-entity read/write scopes are checked again at invocation time.
use super::{routes, transport};
use crate::{App, Json, Path, RequestContext, Result, State, Value, auth, bad, commerce, json};
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
pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let Json(value) = if name == "merchant.customer.groups" {
        commerce::groups(State(a.clone()), h.clone()).await?
    } else {
        let entity = v["entity"]
            .as_str()
            .ok_or(bad("entity required"))?
            .to_string();
        let id = v["id"].as_str().ok_or(bad("id required"))?.to_string();
        match name {
            "merchant.history" => {
                transport::list(
                    State(a.core.clone()),
                    h.clone(),
                    Path((entity, id)),
                    axum::extract::Query(transport::Page {
                        before: v["before"].as_i64(),
                    }),
                )
                .await?
            }
            "merchant.history.version" => {
                transport::detail(
                    State(a.core.clone()),
                    h.clone(),
                    Path((
                        entity,
                        id,
                        v["version"].as_i64().ok_or(bad("version required"))?,
                    )),
                )
                .await?
            }
            "merchant.history.restore" => {
                routes::restore(
                    State(a.clone()),
                    h.clone(),
                    Path((
                        entity,
                        id,
                        v["version"].as_i64().ok_or(bad("version required"))?,
                    )),
                    Json(v.clone()),
                )
                .await?
            }
            _ => return Err(bad("Unknown history operation")),
        }
    };
    Ok(value)
}
