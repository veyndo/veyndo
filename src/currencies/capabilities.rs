//! Currency MCP operations reuse the native HTTP handlers and existing settings/catalog permissions.
use super::routes;
use crate::foundation::App;
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
pub(crate) fn permission(name: &str) -> Option<&'static str> {
    match name {
        "merchant.currencies.refresh" => Some("settings.write"),
        "merchant.currencies.generate" => Some("catalog.write"),
        "merchant.currencies.jobs" | "merchant.currencies.job" => Some("catalog.read"),
        _ => None,
    }
}
pub(crate) fn schema(name: &str) -> Option<Value> {
    let (props, required) = match name {
        "currency.list" => (json!({}), vec![]),
        "currency.select" => (
            json!({"currency":{"type":"string"},"revision":{"type":"integer"}}),
            vec!["currency", "revision"],
        ),
        "merchant.currencies.refresh" => (json!({"revision":{"type":"integer"}}), vec!["revision"]),
        "merchant.currencies.generate" => (
            json!({"currency":{"type":"string"},"revision":{"type":"integer"},"overwrite":{"type":"boolean"}}),
            vec!["currency", "revision"],
        ),
        "merchant.currencies.jobs" => (json!({}), vec![]),
        "merchant.currencies.job" => (json!({"id":{"type":"string"}}), vec!["id"]),
        _ => return None,
    };
    Some(
        json!({"type":"object","properties":props,"required":required,"additionalProperties":false}),
    )
}
/// The JSON fields `invoke` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InvokeInput {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

pub(crate) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let input: InvokeInput = view(v);
    let state = State(a.clone());
    let h = h.clone();
    let Json(result) = match name {
        "currency.list" => {
            use axum::extract::FromRef;
            veyndo_pricing::http::currencies::discover(State(FromRef::from_ref(a)), h).await?
        }
        "currency.select" => routes::select(state, h, Json(v.clone())).await?,
        "merchant.currencies.refresh" => routes::refresh(state, h, Json(v.clone())).await?,
        "merchant.currencies.generate" => {
            use axum::extract::FromRef;
            veyndo_pricing::http::price_jobs::create(
                State(FromRef::from_ref(a)),
                h,
                Json(v.clone()),
            )
            .await?
        }
        "merchant.currencies.jobs" => {
            veyndo_pricing::http::price_jobs::list(State(a.core.clone()), h).await?
        }
        "merchant.currencies.job" => {
            veyndo_pricing::http::price_jobs::detail(
                State(a.core.clone()),
                h,
                Path(input.id.as_deref().ok_or(bad("Job ID required"))?.into()),
            )
            .await?
        }
        _ => return Err(bad("Unknown currency capability")),
    };
    Ok(result)
}
