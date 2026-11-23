//! Conversation attachments come only from the server's executed tool trace, never from model prose or record claims.
use serde_json::{Value, json};

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
            let mut view = json!({
                "attachmentId":index,
                "resourceUri":entry["appView"].as_str()?,
                "arguments":entry["input"],
            });
            if include_result {
                view["result"] = json!({"structuredContent":entry["result"]});
            }
            Some(view)
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
}
