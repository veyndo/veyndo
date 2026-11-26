//! HTTP/MCP POS routes; the POS component owns the cashier gateway behind the register port.
use super::register_host::Register;
use crate::foundation::App;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::merge;
use veyndo_core::request_context::RequestContext;
use veyndo_pos::service::register;
/// The register channel a POS command runs in, written into its arguments.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelContext<'a> {
    channel_id: &'a str,
}
pub(super) async fn channels(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    Ok(Json(register::channels(&a, &h).await?))
}
pub(super) async fn route(
    State(a): State<App>,
    h: RequestContext,
    Path((channel, operation)): Path<(String, String)>,
    Json(mut v): Json<Value>,
) -> Result<Json<Value>> {
    if v.get("channelId").is_some_and(|id| id != &json!(channel)) {
        return Err(bad("POS channel context differs"));
    }
    merge(
        &mut v,
        ChannelContext {
            channel_id: &channel,
        },
    )?;
    Ok(Json(invoke(&a, &h, &operation, &v).await?))
}
pub(crate) async fn invoke(a: &App, h: &RequestContext, op: &str, v: &Value) -> Result<Value> {
    Box::pin(register::invoke(&Register::new(a), a, h, op, v)).await
}

#[cfg(test)]
mod tests {
    use super::ChannelContext;
    use serde_json::json;
    use veyndo_core::document::merge;

    #[test]
    fn channel_context_keeps_the_indexed_shape() {
        for body in [json!({"cartId":"c-1","channelId":"pos-1"}), json!(null)] {
            let mut old = body.clone();
            old["channelId"] = json!("pos-1".to_string());
            let mut new = body;
            merge(
                &mut new,
                ChannelContext {
                    channel_id: "pos-1",
                },
            )
            .unwrap();
            assert_eq!(new, old);
        }
    }
}
