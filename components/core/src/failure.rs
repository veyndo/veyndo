//! Stable safe error details shared by HTTP, MCP and agent streams; legacy constructors retain their wire message.
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, json};
#[derive(Debug)]
pub struct Failure(pub StatusCode, pub String, Problem);
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Problem {
    kind: Kind,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pointer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_revision: Option<i64>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    ValidationError,
    AuthenticationRequired,
    PermissionDenied,
    NotFound,
    Conflict,
    QuotaExceeded,
    RateLimited,
    TemporarilyUnavailable,
    ProviderUnavailable,
    InternalError,
    RevisionConflict,
    OutputContractError,
}
pub type Error = Failure;
/// Compatibility factory for domain errors; structured details never parse human messages.
#[allow(non_snake_case)]
pub fn Error(status: StatusCode, detail: String) -> Failure {
    Failure::new(status, detail)
}
impl Failure {
    /// A spent allocation is a quota, not a rate limit: retrying cannot succeed.
    pub fn exhausted(mut self, pointer: &str) -> Self {
        self.2.kind = Kind::QuotaExceeded;
        self.2.retryable = false;
        self.2.pointer = Some(pointer.into());
        self
    }
    fn new(status: StatusCode, detail: String) -> Self {
        let kind = match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => Kind::ValidationError,
            StatusCode::UNAUTHORIZED => Kind::AuthenticationRequired,
            StatusCode::FORBIDDEN => Kind::PermissionDenied,
            StatusCode::NOT_FOUND => Kind::NotFound,
            StatusCode::CONFLICT => Kind::Conflict,
            StatusCode::PAYLOAD_TOO_LARGE => Kind::QuotaExceeded,
            StatusCode::TOO_MANY_REQUESTS => Kind::RateLimited,
            StatusCode::SERVICE_UNAVAILABLE => Kind::TemporarilyUnavailable,
            StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT => Kind::ProviderUnavailable,
            _ => Kind::InternalError,
        };
        Self(
            status,
            detail,
            Problem {
                kind,
                retryable: matches!(status.as_u16(), 429 | 503),
                scope: None,
                pointer: None,
                current_revision: None,
            },
        )
    }
    /// `scope` is the permission the caller lacks, already resolved to its public name.
    pub fn permission(scope: &str) -> Self {
        let mut error = Self::new(
            StatusCode::FORBIDDEN,
            "Role does not permit this action".into(),
        );
        error.2.scope = Some(scope.into());
        error
    }
    pub fn validation(pointer: &str, detail: String) -> Self {
        let mut error = Self::new(StatusCode::BAD_REQUEST, detail);
        error.2.pointer = Some(pointer.into());
        error
    }
    pub fn revision(current: i64, detail: &str) -> Self {
        let mut error = Self::new(StatusCode::CONFLICT, detail.into());
        error.2.kind = Kind::RevisionConflict;
        error.2.current_revision = Some(current);
        error
    }
    pub fn revision_at(current: i64, pointer: &str, detail: &str) -> Self {
        let mut error = Self::revision(current, detail);
        error.2.pointer = Some(pointer.into());
        error
    }
    pub fn output_contract() -> Self {
        let mut error=Self::new(StatusCode::BAD_GATEWAY,"App result does not match its declared output contract; inspect the original action before retrying".into());
        error.2.kind = Kind::OutputContractError;
        error.2.retryable = false;
        error
    }
    pub fn problem(&self) -> Value {
        let mut problem = json!(self.2);
        problem["code"] = json!(self.0.as_u16().to_string());
        problem["status"] = json!(self.0.as_u16());
        problem["detail"] = json!(self.1);
        problem
    }
    pub fn tool_result(&self) -> Value {
        json!({"content":[{"type":"text","text":self.1}],"isError":true,
            "structuredContent":{"errors":[self.problem()]}})
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let retryable = self.2.retryable;
        let mut response = (self.0, Json(json!({"errors":[self.problem()]}))).into_response();
        if retryable {
            response
                .headers_mut()
                .insert("retry-after", "5".parse().unwrap());
        }
        response
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_allocation_is_not_a_retryable_rate_limit() {
        let error = Error(StatusCode::TOO_MANY_REQUESTS, "Spent".into()).exhausted("/agentBudget");
        assert_eq!(error.problem()["kind"], "quota_exceeded");
        assert_eq!(error.problem()["retryable"], false);
        assert_eq!(error.problem()["pointer"], "/agentBudget");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(!response.headers().contains_key("retry-after"));
    }
    #[test]
    fn structured_failures_keep_legacy_messages_and_explicit_safe_context() {
        let permission = Failure::permission("catalog.write").problem();
        assert_eq!(permission["scope"], "catalog.write");
        assert_eq!(permission["kind"], "permission_denied");
        assert_eq!(
            Failure::revision(7, "Changed").problem()["currentRevision"],
            7
        );
        let result = Failure::validation("/items/0", "Invalid item".into()).tool_result();
        assert_eq!(result["content"][0]["text"], "Invalid item");
        assert_eq!(
            result["structuredContent"]["errors"][0]["pointer"],
            "/items/0"
        );
        assert_eq!(
            Error(StatusCode::TOO_MANY_REQUESTS, "Busy".into()).problem()["retryable"],
            true
        );
        assert_eq!(
            Error(StatusCode::CONFLICT, "Changed".into()).problem()["retryable"],
            false
        );
    }
}
