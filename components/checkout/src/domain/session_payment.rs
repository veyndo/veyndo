//! Classify the opt-in Checkout test payment without introducing a second payment ledger.
use serde_json::Value;
use veyndo_shop::types::cart::StoredCart;

pub fn provider(method: &Value) -> Option<&'static str> {
    if method["mode"] != "app" {
        return None;
    }
    match method["id"].as_str()? {
        "paypal-sandbox" if method["provider"].is_null() => Some("paypal"),
        "shopware-payments-paypal"
            if method["provider"] == "shopware_payments"
                && method["providerMethod"] == "paypal" =>
        {
            Some("shopware_payments")
        }
        _ => None,
    }
}

/// `strict` deployments accept only the provider sandbox, never the contract fixture.
pub fn test_environment(environment: &str, strict: bool) -> bool {
    environment == "sandbox" || (environment == "contract-fixture" && !strict)
}

pub fn attempt(payment: &Value, strict: bool) -> bool {
    provider(&payment["method"]).is_some_and(|provider| payment["provider"] == provider)
        && payment["environment"]
            .as_str()
            .is_some_and(|environment| test_environment(environment, strict))
        && payment["realMoneyCharged"] == false
        && payment["attemptId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
}

pub fn has_saved_funding(cart: &StoredCart) -> bool {
    cart.data
        .checkout
        .as_ref()
        .and_then(|selection| selection.payment_funding.as_ref())
        .is_some_and(|funding| funding.save || funding.id.is_some())
}
