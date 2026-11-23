//! Scenario evidence uses the release owner's configuration projection, never orders or inventory.
use crate::ports::AgentHost;
use serde_json::Value;
use veyndo_core::Result;

pub async fn capture<H: AgentHost>(host: &H, t: &str) -> Option<String> {
    // Evidence is diagnostic: lock contention or the release projection's size
    // limits must not hide a completed purchase or stop an admitted shopper.
    match tokio::time::timeout(std::time::Duration::from_secs(3), snapshot(host, t)).await {
        Ok(Ok(digest)) => Some(digest),
        _ => {
            tracing::warn!(
                tenant = t,
                "Customer test configuration evidence unavailable"
            );
            None
        }
    }
}

/// The digest of the sandbox's release configuration projection.
async fn snapshot<H: AgentHost>(host: &H, t: &str) -> Result<String> {
    host.configuration_digest(t).await
}
pub fn review(evidence: &Value, current: Option<&str>) -> &'static str {
    let Some(started) = evidence["startedDigest"].as_str().filter(|s| !s.is_empty()) else {
        return "unverified";
    };
    let Some(finished) = evidence["finishedDigest"]
        .as_str()
        .filter(|s| !s.is_empty())
    else {
        return "unverified";
    };
    if current.is_none() || evidence["schemaVersion"] != 1 {
        "unverified"
    } else if started != finished {
        "changed_during_run"
    } else if Some(finished) != current {
        "stale"
    } else {
        "current"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn historical_or_partial_evidence_cannot_be_current() {
        for value in [
            Value::Null,
            json!({}),
            json!({"schemaVersion":1,"startedDigest":"a"}),
            json!({"schemaVersion":2,"startedDigest":"a","finishedDigest":"a"}),
        ] {
            assert_eq!(review(&value, Some("a")), "unverified");
        }
    }
    #[test]
    fn configuration_changes_during_execution_and_afterward_are_distinct() {
        let mut value = json!({"schemaVersion":1,"startedDigest":"a","finishedDigest":"a"});
        assert_eq!(review(&value, Some("a")), "current");
        assert_eq!(review(&value, None), "unverified");
        assert_eq!(review(&value, Some("b")), "stale");
        value["finishedDigest"] = json!("b");
        assert_eq!(review(&value, Some("b")), "changed_during_run");
        assert_eq!(review(&value, Some("a")), "changed_during_run");
    }
}
