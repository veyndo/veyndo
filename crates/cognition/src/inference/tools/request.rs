//! Provider-native messages and correlated tool results, including opaque reasoning continuation.
use super::super::{Provider, Value, env, json};
use super::{alias, definitions};
use serde::Deserialize;
use veyndo_core::document::{lenient, merge, view};

/// The native tools a provider request offers.
#[derive(serde::Serialize)]
struct NativeTools {
    tools: Vec<Value>,
}

/// The user input of an OpenAI Responses request before tool continuation wraps it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct WireInput {
    input: Value,
}

/// The Responses members tool continuation sets: the wrapped input and opaque reasoning carry-over.
#[derive(serde::Serialize)]
struct ResponsesContinuation {
    input: Value,
    include: [&'static str; 1],
    parallel_tool_calls: bool,
}

/// A host tool definition, typed as a provider request reads it.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ToolDefinition {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
    input_schema: Value,
}

/// The tool continuation the host records in the request schema.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Continuation {
    #[serde(rename = "x-veyndo-history", deserialize_with = "lenient")]
    history: Option<Vec<Value>>,
}

/// One recorded tool turn: the provider, its opaque assistant output and the correlated results.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Turn {
    provider: Value,
    #[serde(deserialize_with = "lenient")]
    results: Option<Vec<Value>>,
    assistant: Value,
}

/// One correlated tool result.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ToolResult {
    id: Value,
    result: Value,
    alias: Value,
}

pub(in crate::inference) fn request(
    provider: &Provider,
    schema: &Value,
    request: &mut Value,
) -> Result<(), String> {
    if schema.to_string().len() > 300000 {
        return Err("Native tool request budget exceeded".into());
    }
    let defs = definitions(schema)?;
    let chat = matches!(provider, Provider::Openai)
        && env::var("OPENAI_PROTOCOL").as_deref() == Ok("chat");
    let responses = matches!(provider, Provider::Openai) && !chat;
    let tools: Vec<_> = defs.iter().enumerate().map(|(i,t)| {
        let name=alias(i);
        let t: ToolDefinition = view(t);
        let description=format!("{}: {}",t.name.as_deref().unwrap_or(""),t.description.as_deref().unwrap_or("Authorized read"));
        match provider {
            Provider::Anthropic => json!({"name":name,"description":description,"input_schema":t.input_schema}),
            Provider::Openai if responses => json!({"type":"function","name":name,"description":description,"parameters":t.input_schema,"strict":false}),
            _ => json!({"type":"function","function":{"name":name,"description":description,"parameters":t.input_schema}})
        }
    }).collect();
    merge(request, NativeTools { tools }).map_err(|e| e.1)?;
    for key in ["format", "response_format", "output_config", "text"] {
        request
            .as_object_mut()
            .ok_or("Invalid provider request")?
            .remove(key);
    }
    if responses {
        let prompt = view::<WireInput>(request).input;
        let continued = ResponsesContinuation {
            input: json!([{"role":"user","content":prompt}]),
            include: ["reasoning.encrypted_content"],
            parallel_tool_calls: true,
        };
        merge(request, continued).map_err(|e| e.1)?;
    }
    let history = view::<Continuation>(schema)
        .history
        .ok_or("Missing native tool history")?;
    for turn in &history {
        let turn: Turn = view(turn);
        if turn.provider != json!(provider) {
            return Err("Model provider changed during tool continuation".into());
        }
        let results: Vec<ToolResult> = turn
            .results
            .as_ref()
            .ok_or("Invalid native results")?
            .iter()
            .map(view)
            .collect();
        if responses {
            let input = request
                .get_mut("input")
                .and_then(Value::as_array_mut)
                .expect("a responses request input is the list set above");
            input.extend(
                turn.assistant
                    .as_array()
                    .ok_or("Invalid model continuation")?
                    .iter()
                    .cloned(),
            );
            for result in results {
                input.push(json!({"type":"function_call_output","call_id":result.id,"output":result.result.to_string()}));
            }
        } else {
            let messages = request
                .get_mut("messages")
                .and_then(Value::as_array_mut)
                .ok_or("Missing tool messages")?;
            match provider {
                Provider::Anthropic => {
                    messages.push(json!({"role":"assistant","content":turn.assistant}));
                    messages.push(json!({"role":"user","content":results.iter().map(|r|json!({"type":"tool_result","tool_use_id":r.id,"content":r.result.to_string()})).collect::<Vec<_>>()}));
                }
                _ => {
                    messages.push(turn.assistant.clone());
                    for r in results {
                        messages.push(if chat {json!({"role":"tool","tool_call_id":r.id,"content":r.result.to_string()})}else{json!({"role":"tool","tool_name":r.alias,"content":r.result.to_string()})});
                    }
                }
            }
        }
    }
    Ok(())
}
