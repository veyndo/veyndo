//! Self-hosted chat-completions adapter and opt-in provider prompt caching; neither caches private commerce responses.
use super::{Provider, Value, env, json};
use crate::inference::strict_schema;
use serde::Deserialize;
use veyndo_core::document::{lenient, view};
pub(super) fn chat_request(
    model: &str,
    system: &str,
    user: &str,
    schema: &Value,
    images: &[String],
) -> Value {
    let mut content = vec![json!({"type":"text","text":user})];
    content.extend(
        images
            .iter()
            .map(|url| json!({"type":"image_url","image_url":{"url":url}})),
    );
    json!({"model":model,"stream":false,"temperature":0,"max_tokens":8192,"messages":[{"role":"system","content":system},{"role":"user","content":if images.is_empty(){json!(user)}else{json!(content)}}],"response_format":{"type":"json_schema","json_schema":{"name":"commerce_result","strict":true,"schema":strict_schema(schema.clone())}}})
}
/// The JSON fields `chat_response` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ChatResponseInput {
    choices: Value,
}

pub(super) fn chat_response(raw: &Value) -> Result<Value, String> {
    let input: ChatResponseInput = view(raw);
    let choices = input
        .choices
        .as_array()
        .filter(|c| c.len() == 1)
        .ok_or("Invalid chat completions result")?;
    if choices[0]["finish_reason"] != "stop" {
        return Err("Chat completion interrupted or truncated".into());
    }
    serde_json::from_str(
        choices[0]["message"]["content"]
            .as_str()
            .ok_or("Chat response lacks text")?,
    )
    .map_err(|_| "Invalid structured chat response".into())
}
pub(super) fn prompt_cache(provider: &Provider, request: &mut Value, system: &str) {
    if env::var("INFERENCE_PROMPT_CACHE").as_deref() != Ok("true") {
        return;
    }
    match provider {
        Provider::Anthropic => {
            request["system"] =
                json!([{"type":"text","text":system,"cache_control":{"type":"ephemeral"}}])
        }
        Provider::Openai if env::var("OPENAI_PROTOCOL").as_deref() != Ok("chat") => {
            use sha2::{Digest, Sha256};
            request["prompt_cache_key"] = json!(format!(
                "veyndo-system-{:x}",
                Sha256::digest(system.as_bytes())
            ));
        }
        _ => {}
    }
}
/// The provider response fields `parse` reads (Ollama, Anthropic and OpenAI Responses), typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProviderReply {
    done_reason: Value,
    #[serde(deserialize_with = "lenient")]
    message: OllamaMessage,
    stop_reason: Value,
    #[serde(deserialize_with = "lenient")]
    content: Option<Vec<Value>>,
    status: Value,
    #[serde(deserialize_with = "lenient")]
    output: Option<Vec<Value>>,
}

/// The message of an Ollama chat response.
#[derive(Deserialize, Default)]
#[serde(default)]
struct OllamaMessage {
    #[serde(deserialize_with = "lenient")]
    content: Option<String>,
}

/// A content block or output item: its type, text and nested content.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Part {
    #[serde(rename = "type")]
    kind: Value,
    #[serde(deserialize_with = "lenient")]
    text: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content: Option<Vec<Value>>,
}

/// The text of the given blocks of one type, concatenated.
fn text_of(blocks: &[Value], kind: &str) -> String {
    blocks
        .iter()
        .map(view::<Part>)
        .filter(|v| v.kind == kind)
        .filter_map(|v| v.text)
        .collect::<String>()
}

pub(super) fn parse(provider: &Provider, raw: &Value) -> Result<Value, String> {
    let reply: ProviderReply = view(raw);
    let text = match provider {
        Provider::Platform => return Err("Unresolved platform provider".into()),
        Provider::Ollama => {
            if reply.done_reason == "length" {
                return Err("Local model output was truncated".into());
            }
            reply.message.content
        }
        Provider::Anthropic => {
            if reply.stop_reason == "max_tokens" {
                return Err("Model output was truncated".into());
            }
            reply.content.map(|a| text_of(&a, "text"))
        }
        Provider::Openai => {
            if reply.status != "completed" {
                return Err("OpenAI response did not complete".into());
            }
            reply.output.map(|a| {
                a.iter()
                    .filter_map(|v| view::<Part>(v).content)
                    .map(|c| text_of(&c, "output_text"))
                    .collect::<String>()
            })
        }
    }
    .ok_or("Provider response lacks text")?;
    serde_json::from_str(&text).map_err(|_| "Model did not return valid structured output".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_requires_single_completed_structured_result() {
        assert_eq!(
            chat_response(
                &json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"value\":1}"}}]})
            )
            .unwrap(),
            json!({"value":1})
        );
        assert!(
            chat_response(
                &json!({"choices":[{"finish_reason":"length","message":{"content":"{}"}}]})
            )
            .is_err()
        );
        assert!(chat_response(&json!({"choices":[]})).is_err());
    }
}
