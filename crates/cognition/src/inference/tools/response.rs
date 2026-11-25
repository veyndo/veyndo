//! Fail-closed normalization of bounded calls; malformed, truncated or unknown operations never execute.
use super::super::{Provider, Value, env, json};
use super::operation;
use serde::Deserialize;
use veyndo_core::document::{lenient, view};
/// The JSON fields `parse` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ParseInput {
    status: Value,
    output: Value,
    choices: Value,
    #[serde(deserialize_with = "lenient")]
    #[serde(rename = "stop_reason")]
    stop_reason: Option<String>,
    content: Value,
    #[serde(deserialize_with = "lenient")]
    done: Option<bool>,
    #[serde(rename = "done_reason")]
    done_reason: Value,
    message: Value,
}

/// An output item or content block: its type and nested content.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Item {
    #[serde(rename = "type")]
    kind: Value,
    #[serde(deserialize_with = "lenient")]
    content: Option<Vec<Value>>,
}

/// One chat completion choice.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Choice {
    #[serde(deserialize_with = "lenient")]
    finish_reason: Option<String>,
    message: Value,
}

/// An assistant message: its refusal and requested tool calls.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Message {
    refusal: Value,
    #[serde(deserialize_with = "lenient")]
    tool_calls: Option<Vec<Value>>,
}

/// A provider tool call; `function` is present for chat-style calls, even when null.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Call {
    #[serde(deserialize_with = "present")]
    function: Option<Value>,
    input: Value,
    #[serde(deserialize_with = "lenient")]
    call_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

/// The name and arguments of a called function.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Function {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    arguments: Value,
}

/// The operation a tool alias resolves to.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Operation {
    name: Value,
}

/// Field deserializer: any present value, null included.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Ok(Some(Value::deserialize(d)?))
}

pub(in crate::inference) fn parse(
    provider: &Provider,
    schema: &Value,
    raw: &Value,
) -> Result<(Value, Value), String> {
    let args: ParseInput = view(raw);
    let chat = matches!(provider, Provider::Openai)
        && env::var("OPENAI_PROTOCOL").as_deref() == Ok("chat");
    let (assistant, calls) = match provider {
        Provider::Openai if !chat => {
            if args.status != "completed" {
                return Err("Tool response did not complete".into());
            }
            let items = args.output.as_array().ok_or("Invalid tool output")?;
            if items.iter().map(view::<Item>).any(|v| {
                v.kind == "message"
                    && v.content
                        .is_some_and(|c| c.iter().any(|v| view::<Item>(v).kind == "refusal"))
            }) {
                return Err("Model refused the tool request".into());
            }
            (
                json!(items),
                items
                    .iter()
                    .filter(|v| view::<Item>(v).kind == "function_call")
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        }
        Provider::Openai => {
            let choices = args
                .choices
                .as_array()
                .filter(|v| v.len() == 1)
                .ok_or("Invalid tool choices")?;
            let choice: Choice = view(&choices[0]);
            if !["stop", "tool_calls"].contains(&choice.finish_reason.as_deref().unwrap_or("")) {
                return Err("Tool response interrupted".into());
            }
            let message: Message = view(&choice.message);
            if !message.refusal.is_null() {
                return Err("Model refused the tool request".into());
            }
            (choice.message, message.tool_calls.unwrap_or_default())
        }
        Provider::Anthropic => {
            if !["end_turn", "tool_use"].contains(&args.stop_reason.as_deref().unwrap_or("")) {
                return Err("Tool response interrupted".into());
            }
            let items = args.content.as_array().ok_or("Invalid tool content")?;
            (
                json!(items),
                items
                    .iter()
                    .filter(|v| view::<Item>(v).kind == "tool_use")
                    .cloned()
                    .collect(),
            )
        }
        Provider::Ollama => {
            if args.done != Some(true) || args.done_reason == "length" {
                return Err("Local tool response interrupted".into());
            }
            let message: Message = view(&args.message);
            (args.message, message.tool_calls.unwrap_or_default())
        }
        Provider::Platform => return Err("Unresolved tool provider".into()),
    };
    if calls.len() > 3 {
        return Err("Model exceeded native tool-call budget".into());
    }
    let mut normalized = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (i, call) in calls.iter().enumerate() {
        let fields: Call = view(call);
        let function: Function = view(fields.function.as_ref().unwrap_or(call));
        let name = function.name.as_deref().ok_or("Missing native tool name")?;
        let operation: Operation = view(operation(schema, name)?);
        let input = if matches!(provider, Provider::Anthropic) {
            fields.input
        } else if let Some(s) = function.arguments.as_str() {
            serde_json::from_str(s).map_err(|_| "Invalid native tool arguments")?
        } else {
            function.arguments
        };
        if !input.is_object() || input.to_string().len() > 8192 {
            return Err("Invalid or oversized native tool arguments".into());
        }
        let correlation = fields.call_id.or(fields.id);
        if !matches!(provider, Provider::Ollama) && correlation.is_none() {
            return Err("Provider tool call lacks its correlation ID".into());
        }
        let id = correlation.unwrap_or_else(|| format!("local_{i}"));
        if id.is_empty() || id.len() > 200 || !ids.insert(id.clone()) {
            return Err("Invalid or duplicate tool call ID".into());
        }
        normalized.push(
            json!({"id":id,"name":operation.name,"arguments_json":input.to_string(),"alias":name}),
        );
    }
    Ok((json!({"tool_calls":normalized}), assistant))
}
