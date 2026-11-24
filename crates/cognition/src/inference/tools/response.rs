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
            if items.iter().any(|v| {
                v["type"] == "message"
                    && v["content"]
                        .as_array()
                        .is_some_and(|c| c.iter().any(|v| v["type"] == "refusal"))
            }) {
                return Err("Model refused the tool request".into());
            }
            (
                json!(items),
                items
                    .iter()
                    .filter(|v| v["type"] == "function_call")
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
            if !["stop", "tool_calls"].contains(&choices[0]["finish_reason"].as_str().unwrap_or(""))
            {
                return Err("Tool response interrupted".into());
            }
            let message = &choices[0]["message"];
            if !message["refusal"].is_null() {
                return Err("Model refused the tool request".into());
            }
            (
                message.clone(),
                message["tool_calls"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            )
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
                    .filter(|v| v["type"] == "tool_use")
                    .cloned()
                    .collect(),
            )
        }
        Provider::Ollama => {
            if args.done != Some(true) || args.done_reason == "length" {
                return Err("Local tool response interrupted".into());
            }
            let message = &args.message;
            (
                message.clone(),
                message["tool_calls"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            )
        }
        Provider::Platform => return Err("Unresolved tool provider".into()),
    };
    if calls.len() > 3 {
        return Err("Model exceeded native tool-call budget".into());
    }
    let mut normalized = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (i, call) in calls.iter().enumerate() {
        let function = if call.get("function").is_some() {
            &call["function"]
        } else {
            call
        };
        let name = function["name"]
            .as_str()
            .ok_or("Missing native tool name")?;
        let operation = operation(schema, name)?;
        let input = if matches!(provider, Provider::Anthropic) {
            call["input"].clone()
        } else if let Some(s) = function["arguments"].as_str() {
            serde_json::from_str(s).map_err(|_| "Invalid native tool arguments")?
        } else {
            function["arguments"].clone()
        };
        if !input.is_object() || input.to_string().len() > 8192 {
            return Err("Invalid or oversized native tool arguments".into());
        }
        let id = call["call_id"]
            .as_str()
            .or_else(|| call["id"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("local_{i}"));
        if !matches!(provider, Provider::Ollama)
            && call["call_id"]
                .as_str()
                .or_else(|| call["id"].as_str())
                .is_none()
        {
            return Err("Provider tool call lacks its correlation ID".into());
        }
        if id.is_empty() || id.len() > 200 || !ids.insert(id.clone()) {
            return Err("Invalid or duplicate tool call ID".into());
        }
        normalized.push(json!({"id":id,"name":operation["name"],"arguments_json":input.to_string(),"alias":name}));
    }
    Ok((json!({"tool_calls":normalized}), assistant))
}
