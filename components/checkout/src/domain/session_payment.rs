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
#[cfg(test)]
mod tests {
    use super::{attempt, has_saved_funding, provider, test_environment};
    use serde_json::json;
    use veyndo_shop::types::cart::StoredCart;
    use veyndo_shop::types::settings::{CheckoutSelection, FundingChoice};

    fn cart(admin: Option<&str>) -> StoredCart {
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(json!({"items":[],"group":"retail","session":"s",
                "customer_id":"customer-1","locale":"de-DE","admin_actor":admin}))
            .expect("cart"),
            revision: 3,
            status: "open".into(),
        }
    }

    #[test]
    fn only_the_two_test_methods_name_a_provider() {
        assert_eq!(
            provider(&json!({"mode":"app","id":"paypal-sandbox"})),
            Some("paypal")
        );
        assert_eq!(
            provider(&json!({"mode":"app","id":"shopware-payments-paypal",
                "provider":"shopware_payments","providerMethod":"paypal"})),
            Some("shopware_payments")
        );
        for method in [
            json!({"mode":"simulated","id":"paypal-sandbox"}),
            json!({"mode":"app","id":"paypal-sandbox","provider":"other"}),
            json!({"mode":"app","id":"paypal-live"}),
            json!({"mode":"app","id":"shopware-payments-paypal","provider":"shopware_payments","providerMethod":"card"}),
            json!({"mode":"app"}),
        ] {
            assert_eq!(provider(&method), None, "{method}");
        }
    }

    #[test]
    fn strict_deployments_accept_only_the_provider_sandbox() {
        assert!(test_environment("sandbox", true) && test_environment("sandbox", false));
        assert!(test_environment("contract-fixture", false));
        assert!(!test_environment("contract-fixture", true));
        assert!(!test_environment("live", false));
    }

    #[test]
    fn a_test_attempt_charges_no_real_money_and_names_its_attempt() {
        let payment = json!({"method":{"mode":"app","id":"paypal-sandbox"},"provider":"paypal",
            "environment":"sandbox","realMoneyCharged":false,"attemptId":"attempt-1"});
        assert!(attempt(&payment, true));
        for (key, value) in [
            ("provider", json!("shopware_payments")),
            ("environment", json!("live")),
            ("realMoneyCharged", json!(true)),
            ("attemptId", json!("")),
        ] {
            let mut changed = payment.clone();
            changed[key] = value;
            assert!(!attempt(&changed, true), "{key}");
        }
        let mut fixture = payment;
        fixture["environment"] = json!("contract-fixture");
        assert!(attempt(&fixture, false) && !attempt(&fixture, true));
    }

    #[test]
    fn saved_funding_is_a_saved_method_or_the_wish_to_save_one() {
        let mut c = cart(None);
        assert!(!has_saved_funding(&c));
        for (funding, expected) in [
            (
                FundingChoice {
                    save: false,
                    id: None,
                },
                false,
            ),
            (
                FundingChoice {
                    save: true,
                    id: None,
                },
                true,
            ),
            (
                FundingChoice {
                    save: false,
                    id: Some("method-1".into()),
                },
                true,
            ),
        ] {
            c.data.checkout = Some(CheckoutSelection {
                payment_funding: Some(funding),
                ..CheckoutSelection::defaults()
            });
            assert_eq!(has_saved_funding(&c), expected);
        }
    }
}
