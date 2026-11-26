//! Capture timing of a payment method: capture at once or authorize first, as configured and as the provider method supports.
use veyndo_app_model::payment_provider::ProviderMethod;
use veyndo_core::{Result, bad};

/// The capture intent of a method: the configured mode or the method default, when the method supports it.
pub fn intent<'a>(configured: Option<&'a str>, method: &'a ProviderMethod) -> Result<&'a str> {
    let intent = configured.unwrap_or(&method.intent);
    if !["capture", "authorize"].contains(&intent)
        || !method.capabilities.iter().any(|c| c == intent)
    {
        return Err(bad("Capture mode is not supported by this payment method"));
    }
    Ok(intent)
}

#[cfg(test)]
mod tests {
    use super::intent;
    use serde_json::json;
    use veyndo_app_model::payment_provider::ProviderMethod;

    fn method(default: &str, capabilities: &[&str]) -> ProviderMethod {
        serde_json::from_value(json!({"id":"card","name":{"en":"Card"},"currencies":["EUR"],"countries":["DE"],"capabilities":capabilities,"checkout":"redirect","intent":default})).unwrap()
    }

    #[test]
    fn the_configured_intent_must_be_a_supported_capture_mode() {
        let card = method("capture", &["capture", "authorize"]);
        assert_eq!(intent(None, &card).unwrap(), "capture");
        assert_eq!(intent(Some("authorize"), &card).unwrap(), "authorize");
        assert!(intent(Some("refund"), &card).is_err());
        let instant = method("capture", &["capture"]);
        assert!(intent(Some("authorize"), &instant).is_err());
    }
}
