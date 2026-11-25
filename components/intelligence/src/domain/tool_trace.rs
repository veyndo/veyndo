//! Conversation attachments come only from the server's executed tool trace, never from model prose or record claims.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};

/// A read transcript entry with the app view it returned.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct TraceEntry {
    #[serde(deserialize_with = "lenient")]
    app_view: Option<String>,
    input: Value,
    result: Value,
}

/// A conversation attachment: the app view a read returned, its arguments and, for a live answer, its result.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Attachment {
    attachment_id: usize,
    resource_uri: String,
    arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<StructuredResult>,
}

/// An app tool result as the conversation view consumes it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuredResult {
    structured_content: Value,
}

/// Only the server's executed tool trace can produce a conversation attachment.
pub fn attachments(trace: &[Value]) -> Vec<Value> {
    attachment_views(trace, true)
}

/// The views of a trace without their results, for stored conversations.
pub fn attachment_references(trace: &[Value]) -> Vec<Value> {
    attachment_views(trace, false)
}

fn attachment_views(trace: &[Value], include_result: bool) -> Vec<Value> {
    trace
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(index, entry)| {
            let entry = view::<TraceEntry>(entry);
            Some(json!(Attachment {
                attachment_id: index,
                resource_uri: entry.app_view?,
                arguments: entry.input,
                result: include_result.then_some(StructuredResult {
                    structured_content: entry.result,
                }),
            }))
        })
        .take(3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_executed_view_calls_become_attachments_and_at_most_three() {
        let trace: Vec<_> = (0..5)
            .map(|i| json!({"appView": (i != 1).then(|| format!("ui://veyndo/{i}")), "input": {"n": i}, "result": {"ok": i}}))
            .collect();
        let views = attachments(&trace);
        assert_eq!(views.len(), 3);
        assert_eq!(views[0]["attachmentId"], 4);
        assert_eq!(views[0]["result"]["structuredContent"]["ok"], 4);
        assert_eq!(views[2]["attachmentId"], 2);
        let references = attachment_references(&trace);
        assert_eq!(references.len(), 3);
        assert!(references[0].get("result").is_none());
        assert!(attachments(&[json!({"input": {}})]).is_empty());
    }
    #[test]
    fn attachments_keep_the_indexed_shape() {
        let trace = vec![json!({"appView": "ui://a", "input": {"q": 1}, "result": {"x": [1]}})];
        let mut old = json!({"attachmentId": 0, "resourceUri": "ui://a", "arguments": {"q": 1}});
        let reference = old.clone();
        old["result"] = json!({"structuredContent": {"x": [1]}});
        assert_eq!(attachments(&trace)[0].to_string(), old.to_string());
        assert_eq!(
            attachment_references(&trace)[0].to_string(),
            reference.to_string()
        );
    }
}
