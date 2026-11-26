//! JSON-RPC envelope errors differ from actionable domain/tool failures; no transport invents authorization.
use crate::foundation::App;
use crate::ucp::keep_present;
use crate::{apps, capabilities};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_core::Error;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
pub(super) struct RpcError {
    code: i32,
    error: Error,
}
impl RpcError {
    pub(super) fn invalid(message: &str) -> Self {
        Self {
            code: -32602,
            error: bad(message),
        }
    }
    pub(super) fn method() -> Self {
        Self {
            code: -32601,
            error: bad("Method not supported"),
        }
    }
    pub(super) fn server(error: Error) -> Self {
        Self {
            code: -32603,
            error,
        }
    }
    pub(super) fn response(self, id: Value) -> Value {
        json!({"jsonrpc":"2.0","id":id,"error":{"code":self.code,"message":self.error.1,"data":self.error.problem()}})
    }
}
/// The JSON-RPC 2.0 request envelope fields.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Envelope {
    jsonrpc: Value,
    #[serde(deserialize_with = "lenient")]
    method: Option<String>,
    id: Value,
    #[serde(deserialize_with = "keep_present")]
    params: Option<Value>,
}
pub(super) fn envelope(value: &Value) -> std::result::Result<(Value, String), RpcError> {
    let request: Envelope = view(value);
    if !value.is_object()
        || request.jsonrpc != "2.0"
        || request.method.as_deref().is_none_or(str::is_empty)
        || !(request.id.is_null() || request.id.is_string() || request.id.is_number())
        || request.params.is_some_and(|params| !params.is_object())
    {
        return Err(RpcError {
            code: -32600,
            error: bad("Invalid JSON-RPC 2.0 request"),
        });
    }
    Ok((request.id, request.method.unwrap_or_default()))
}
/// The `tools/call` parameters; `arguments` is the tool's open input, checked by the tool's own contract.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CallParams {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "keep_present")]
    arguments: Option<Value>,
    #[serde(rename = "_meta", deserialize_with = "lenient")]
    meta: CallMeta,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct CallMeta {
    #[serde(rename = "ai.veyndo/resourceUri", deserialize_with = "lenient")]
    resource_uri: Option<String>,
}
/// The name of an app tool descriptor.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ToolName {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
}
/// A successful tool result: the value as text and as structured content.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolResult {
    content: [TextContent; 1],
    structured_content: Value,
    is_error: bool,
}
#[derive(Serialize)]
struct TextContent {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}
impl ToolResult {
    fn json(value: Value) -> Value {
        serde_json::to_value(Self {
            content: [TextContent {
                kind: "text",
                text: value.to_string(),
            }],
            structured_content: value,
            is_error: false,
        })
        .expect("Tool results serialize")
    }
}
pub(super) async fn call(
    a: &App,
    h: &RequestContext,
    params: &Value,
) -> std::result::Result<Value, RpcError> {
    let params: CallParams = view(params);
    let name = params
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| RpcError::invalid("Tool name required"))?;
    let input = params.arguments.unwrap_or_else(|| json!({}));
    let input = &input;
    if !input.is_object() {
        return Err(RpcError::invalid("Tool arguments must be an object"));
    }
    let native = capabilities::OPERATIONS
        .iter()
        .any(|operation| operation.name == name);
    if !native
        && (!name.starts_with("app.")
            || !apps::app_tools(a, h)
                .await
                .map_err(RpcError::server)?
                .iter()
                .any(|tool| view::<ToolName>(tool).name.as_deref() == Some(name)))
    {
        return Err(RpcError::invalid("Unknown or unavailable tool"));
    }
    let result = if let Some(uri) = params.meta.resource_uri.as_deref() {
        // A first-party iframe can only rerun the single read tool of its pinned view.
        apps::invoke_mcp_read(a, h, name, input, Some(uri)).await
    } else {
        super::transport::invoke_transport(a, h, name, input).await
    };
    Ok(match result {
        Ok(value) => ToolResult::json(value),
        Err(error) => error.tool_result(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn call_params_and_results_match_the_indexed_shape() {
        let value = json!({"a":[1,"x"]});
        assert_eq!(
            ToolResult::json(value.clone()),
            json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":false})
        );
        for params in [
            json!({}),
            json!({"name":"t","arguments":null,"_meta":"x"}),
            json!({"name":7,"arguments":{"a":1},"_meta":{"ai.veyndo/resourceUri":"ui://v"}}),
        ] {
            let typed: CallParams = view(&params);
            assert_eq!(typed.name.as_deref(), params["name"].as_str());
            assert_eq!(typed.arguments.as_ref(), params.get("arguments"));
            assert_eq!(
                typed.meta.resource_uri.as_deref(),
                params["_meta"]["ai.veyndo/resourceUri"].as_str()
            );
        }
    }
    #[test]
    fn request_errors_are_distinct_and_null_id_is_a_request() {
        for request in [
            json!([]),
            json!({"jsonrpc":"2.0","id":{},"method":"ping"}),
            json!({"jsonrpc":"2.0","method":true}),
            json!({"jsonrpc":"2.0","method":"ping","params":[]}),
        ] {
            assert_eq!(
                envelope(&request).err().unwrap().response(Value::Null)["error"]["code"],
                -32600
            );
        }
        assert!(envelope(&json!({"jsonrpc":"2.0","method":"ping","id":null})).is_ok());
        assert!(envelope(&json!({"jsonrpc":"2.0","method":"ping","id":1,"params":null})).is_err());
        assert_eq!(
            envelope(&json!({"jsonrpc":"2.0","method":"tools/call","id":"a","params":{}}))
                .ok()
                .unwrap(),
            (json!("a"), "tools/call".to_owned())
        );
        assert_eq!(
            RpcError::method().response(json!(1))["error"]["code"],
            -32601
        );
        assert_eq!(
            RpcError::invalid("Unknown").response(json!(1))["error"]["code"],
            -32602
        );
        assert_eq!(
            RpcError::server(bad("Failed")).response(json!(1))["error"]["code"],
            -32603
        );
    }
}
