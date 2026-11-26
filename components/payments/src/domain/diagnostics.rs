//! Safe, provider-neutral operational hints. These never authorize a financial transition.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};

pub fn code(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("PAYMENT_SAVED_METHODS_UNAVAILABLE") => Some("PAYMENT_SAVED_METHODS_UNAVAILABLE"),
        Some("PAYMENT_METHOD_UNAVAILABLE") => Some("PAYMENT_METHOD_UNAVAILABLE"),
        Some("PAYMENT_PROVIDER_CONFIGURATION_REQUIRED") => {
            Some("PAYMENT_PROVIDER_CONFIGURATION_REQUIRED")
        }
        _ => None,
    }
}

/// The JSON fields `response` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ResponseInput {
    #[serde(deserialize_with = "lenient")]
    error: Option<String>,
}

pub fn response(value: &Value) -> &'static str {
    let input: ResponseInput = view(value);
    code(input.error.as_deref()).unwrap_or("Payment service rejected operation")
}

#[cfg(test)]
mod tests {
    use super::{code, response};
    use serde_json::{Value, json};
    #[test]
    fn diagnostics_accept_only_exact_public_codes() {
        assert_eq!(
            response(&json!({"error":"PAYMENT_SAVED_METHODS_UNAVAILABLE","details":"secret"})),
            "PAYMENT_SAVED_METHODS_UNAVAILABLE"
        );
        for v in [
            json!({"error":"private provider token"}),
            json!({"error":"PAYMENT_METHOD_UNAVAILABLE\nsecret"}),
            json!({"error":{"code":"PAYMENT_METHOD_UNAVAILABLE"}}),
            Value::Null,
        ] {
            assert_eq!(response(&v), "Payment service rejected operation");
        }
        assert_eq!(code(Some("SQL customer@example.test")), None);
    }
}
