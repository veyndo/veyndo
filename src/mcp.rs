//! Typed MCP schemas and JSON-RPC transport.
use crate::apps;
use crate::capabilities::OPERATIONS;
use crate::foundation::App;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::env;
use veyndo_core::Error;
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::request_context::RequestContext;
use veyndo_core::shop_scope::header;
pub(crate) mod poppy;
mod protocol;
mod storefront_views;
mod transport;
use protocol::RpcError;
/// The JSON-RPC request fields this endpoint reads besides its envelope.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RpcRequest {
    #[serde(deserialize_with = "lenient")]
    params: Raw<RpcParams>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RpcParams {
    protocol_version: Value,
    uri: Value,
    name: Value,
    #[serde(deserialize_with = "lenient")]
    capabilities: ClientCapabilities,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ClientCapabilities {
    #[serde(deserialize_with = "lenient")]
    extensions: serde_json::Map<String, Value>,
}

/// The configured public HTTPS origin of this deployment, if any.
fn public_origin() -> Option<String> {
    env::var("COMMERCE_PUBLIC_ORIGIN").ok().and_then(|v| {
        reqwest::Url::parse(&v)
            .ok()
            .filter(|u| {
                u.scheme() == "https"
                    && u.username().is_empty()
                    && u.password().is_none()
                    && u.path() == "/"
                    && u.query().is_none()
                    && u.fragment().is_none()
            })
            .map(|u| u.origin().ascii_serialization())
    })
}

pub(crate) async fn mcp(State(a): State<App>, h: RequestContext, Json(v): Json<Value>) -> Response {
    let public = public_origin();
    if let Some(origin) = header(&h, "origin") {
        // Trust only explicit deployment configuration, never Host or forwarded headers.
        let bind = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
        let local = bind
            .parse::<std::net::SocketAddr>()
            .ok()
            .filter(|b| b.ip().is_loopback());
        let loopback = local.is_some_and(|b| {
            origin == format!("http://{}", b) || origin == format!("http://localhost:{}", b.port())
        });
        if public.as_deref() != Some(origin) && !loopback {
            return Error(StatusCode::FORBIDDEN, "Origin rejected".into()).into_response();
        }
    }
    let (id, method) = match protocol::envelope(&v) {
        Ok(request) => request,
        Err(error) => return Json(error.response(Value::Null)).into_response(),
    };
    if v.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let request: RpcRequest = view(&v);
    let params = &request.params;
    let result = match method {
        "initialize" => {
            let mut capabilities = json!({"tools":{},"resources":{}});
            if params
                .capabilities
                .extensions
                .get(apps::mcp_views::EXTENSION)
                .and_then(|extension| extension.get("mimeTypes"))
                .and_then(Value::as_array)
                .is_some_and(|types| types.iter().any(|mime| mime == apps::mcp_views::MIME))
            {
                capabilities["extensions"] = json!({apps::mcp_views::EXTENSION:{}});
            }
            Ok(json!({"protocolVersion":if params.protocol_version=="2025-11-25"{"2025-11-25"}else{"2026-07-28"},"capabilities":capabilities,"serverInfo":{"name":"veyndo","version":env!("CARGO_PKG_VERSION")}}))
        }
        "ping" => Ok(json!({})),
        "tools/list" => {
            let mut tools = OPERATIONS
                .iter()
                .filter(|operation| operation.admitted(&a, &h))
                .map(|operation| {
                    let mut tool = operation.tool();
                    storefront_views::link(operation.name, &mut tool);
                    tool
                })
                .collect::<Vec<_>>();
            match apps::app_tools(&a, &h).await {
                Ok(apps) => {
                    tools.extend(apps);
                    Ok(json!({"tools":tools}))
                }
                Err(e) => Err(RpcError::server(e)),
            }
        }
        "tools/call" => protocol::call(&a, &h, &params.raw).await.map(|mut result| {
            if let Some(name) = params.name.as_str() {
                storefront_views::attach(&h, name, &mut result, public.as_deref());
            }
            result
        }),
        "resources/list" => apps::mcp_views::list(&a, &h)
            .await
            .map(|mut resources| {
                resources.insert(0, storefront_views::listing());
                resources.insert(0, json!({"uri":"commerce://capabilities","name":"Commerce capabilities","mimeType":"application/json"}));
                json!({"resources":resources})
            })
            .map_err(RpcError::server),
        "resources/read" if params.uri == "commerce://capabilities" => Ok(
            json!({"contents":[{"uri":"commerce://capabilities","mimeType":"application/json","text":json!({"capabilities": OPERATIONS.iter().map(|operation|operation.description()).collect::<Vec<_>>()}).to_string()}]}),
        ),
        "resources/read" if params.uri == storefront_views::URI => {
            storefront_views::read(public.as_deref())
                .await
                .map(|content| json!({"contents":[content]}))
                .ok_or_else(|| RpcError::invalid("Resource not found"))
        }
        "resources/read" => match params.uri.as_str() {
            Some(uri) => apps::mcp_views::read(&a, &h, uri).await
                .map_err(RpcError::server)
                .and_then(|resource| resource
                    .map(|content| json!({"contents":[content]}))
                    .ok_or_else(|| RpcError::invalid("Resource not found"))),
            None => Err(RpcError::invalid("Resource not found")),
        },
        _ => Err(RpcError::method()),
    };
    Json(match result {
        Ok(r) => json!({"jsonrpc":"2.0","id":id,"result":r}),
        Err(e) => e.response(id),
    })
    .into_response()
}
