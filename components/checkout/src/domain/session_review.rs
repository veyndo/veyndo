//! The purchase a buyer explicitly reviewed in a checkout session, and the checks that bind the order to exactly that review.
use crate::domain::session_payment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{Raw, lenient, merge, view};
use veyndo_core::{Error, Result, StatusCode, bad, conflict};
use veyndo_shop::types::cart::StoredCart;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub session_id: String,
    pub cart_id: String,
    pub customer_id: String,
    cart_revision: i64,
    locale: String,
    currency: String,
    country: String,
    payment_method: String,
    legal_policy_version: String,
    total_minor: String,
    pub confirmed: bool,
    #[serde(default)]
    pub paypal_sandbox: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recovery {
    pub session_id: String,
    pub cart_id: String,
    pub customer_id: String,
}

/// The idempotency key of a session: one session places at most one order.
pub fn key(session: &str) -> Result<String> {
    let id = uuid::Uuid::parse_str(session).map_err(|_| bad("Invalid checkout session ID"))?;
    if session != id.to_string() {
        return Err(bad("Invalid checkout session ID"));
    }
    Ok(format!("checkout-session-v1:{id}"))
}

/// The placed order facts a checkout session compares and reports; every field keeps its stored JSON value.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewedOrder {
    checkout_session_id: Value,
    id: Value,
    order_number: Value,
    state: Value,
    #[serde(deserialize_with = "lenient")]
    cart: Raw<CartFacts>,
    #[serde(deserialize_with = "lenient")]
    order_customer: OrderCustomer,
    #[serde(deserialize_with = "lenient")]
    payment: Raw<PaymentFacts>,
    #[serde(deserialize_with = "lenient")]
    legal: LegalFacts,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CartFacts {
    id: Value,
    #[serde(deserialize_with = "lenient")]
    price: PriceFacts,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PriceFacts {
    currency: Value,
    total_minor: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct OrderCustomer {
    customer_id: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PaymentFacts {
    provider: Value,
    real_money_charged: Value,
    state: Value,
    attempt_id: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LegalFacts {
    policy_version: Value,
}

/// The quote facts a reviewed checkout must still match.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewedQuote {
    revision: Value,
    #[serde(deserialize_with = "lenient")]
    price: PriceFacts,
    #[serde(deserialize_with = "lenient")]
    checkout: CheckoutFacts,
    #[serde(deserialize_with = "lenient")]
    payment_method: Raw<MethodFacts>,
    selection_needs_confirmation: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CheckoutFacts {
    country: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct MethodFacts {
    mode: Value,
    id: Value,
}

/// The sandbox payment attempt a receipt names.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SandboxAttempt<'a> {
    payment_attempt_id: &'a Value,
}

/// What the session service learns about its order; `strict` excludes the contract fixture as a test environment.
pub fn receipt(
    order: &Value,
    session: &str,
    cart: &str,
    customer: &str,
    strict: bool,
) -> Result<Value> {
    let order: ReviewedOrder = view(order);
    let payment = &order.payment;
    let sandbox_attempt = session_payment::attempt(&payment.raw, strict);
    if order.checkout_session_id != session
        || order.cart.id != cart
        || order.order_customer.customer_id != customer
        || !(sandbox_attempt
            || (payment.provider == "simulated" && payment.real_money_charged == false))
    {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Checkout receipt unavailable".into(),
        ));
    }
    let mut result = json!({
        "sessionId": session,
        "cartId": cart,
        "customerId": customer,
        "orderId": order.id,
        "orderNumber": order.order_number,
        "orderState": order.state,
        "paymentState": payment.state,
        "currency": order.cart.price.currency,
        "totalMinor": order.cart.price.total_minor,
    });
    if sandbox_attempt {
        merge(
            &mut result,
            SandboxAttempt {
                payment_attempt_id: &payment.attempt_id,
            },
        )?;
    }
    Ok(result)
}

impl Review {
    pub fn session(&self) -> &str {
        &self.session_id
    }

    /// The locked cart is still the reviewed one: same buyer, language and channel, and an ordinary customer cart.
    pub fn admit_cart(&self, c: &StoredCart, channel: &str, buyer: &str) -> Result<()> {
        if c.id != self.cart_id
            || c.data.customer_id.as_deref() != Some(&self.customer_id)
            || !c.data.locale.eq_ignore_ascii_case(&self.locale)
            || c.data.sales_channel != channel
            || c.data.admin_actor.is_some()
            || c.data.pos_actor.is_some()
        {
            return Err(conflict("Checkout cart context changed; review again"));
        }
        if buyer != self.customer_id {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Current buyer is required".into(),
            ));
        }
        if self.paypal_sandbox && session_payment::has_saved_funding(c) {
            return Err(conflict(
                "Use a new PayPal Sandbox payment for this checkout",
            ));
        }
        Ok(())
    }

    pub fn check_quote(&self, q: &Value, policy: &Value, minor: i64) -> Result<()> {
        let q: ReviewedQuote = view(q);
        let method = &q.payment_method;
        let allowed = if self.paypal_sandbox {
            session_payment::provider(&method.raw).is_some()
        } else {
            method.mode == "simulated"
        };
        if !allowed {
            return Err(conflict(
                "The reviewed payment method is unavailable for this checkout profile",
            ));
        }
        if q.revision != self.cart_revision
            || q.price.currency != self.currency
            || q.checkout.country != self.country
            || method.id != self.payment_method
            || policy != &self.legal_policy_version
            || minor.to_string() != self.total_minor
            || q.selection_needs_confirmation == true
        {
            return Err(conflict("Checkout changed; review your order again"));
        }
        Ok(())
    }

    pub fn check_replay(&self, order: &Value, strict: bool) -> Result<()> {
        receipt(
            order,
            &self.session_id,
            &self.cart_id,
            &self.customer_id,
            strict,
        )?;
        let order: ReviewedOrder = view(order);
        if self.paypal_sandbox != session_payment::attempt(&order.payment.raw, strict) {
            return Err(conflict("Review the original checkout payment profile"));
        }
        let total = order
            .cart
            .price
            .total_minor
            .as_str()
            .and_then(|value| value.parse().ok())
            .ok_or(conflict("Checkout receipt amount unavailable"))?;
        self.check_quote(&order.cart.raw, &order.legal.policy_version, total)
    }

    pub fn check_prepared_payment(&self, payment: &Value, strict: bool) -> Result<()> {
        if self.paypal_sandbox && session_payment::attempt(payment, strict) {
            Ok(())
        } else {
            Err(conflict("Use a PayPal Sandbox account for this checkout"))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{Review, key, receipt};
    use serde_json::{Value, json};
    use veyndo_core::StatusCode;
    use veyndo_shop::types::cart::StoredCart;
    use veyndo_shop::types::settings::{CheckoutSelection, FundingChoice};

    const SESSION: &str = "3f2b8c1e-5a4d-4e6f-8a9b-0c1d2e3f4a5b";

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

    fn review(paypal: bool) -> Review {
        serde_json::from_value(json!({"sessionId":SESSION,"cartId":"cart-1","customerId":"customer-1",
            "cartRevision":3,"locale":"de-DE","currency":"EUR","country":"DE",
            "paymentMethod":if paypal {"paypal-sandbox"} else {"simulated"},
            "legalPolicyVersion":"policy-7","totalMinor":"1000","confirmed":true,"paypalSandbox":paypal}))
        .expect("review")
    }

    fn quote(paypal: bool) -> Value {
        json!({"id":"cart-1","revision":3,"price":{"currency":"EUR","totalMinor":"1000"},
            "checkout":{"country":"DE"},
            "paymentMethod":if paypal {json!({"mode":"app","id":"paypal-sandbox"})} else {json!({"mode":"simulated","id":"simulated"})}})
    }

    fn placed(paypal: bool) -> Value {
        let payment = if paypal {
            json!({"method":{"mode":"app","id":"paypal-sandbox"},"provider":"paypal","environment":"sandbox",
                "realMoneyCharged":false,"attemptId":"attempt-1","state":"pending"})
        } else {
            json!({"provider":"simulated","realMoneyCharged":false,"state":"authorized"})
        };
        json!({"id":"order-1","orderNumber":"RAC-1","state":"placed","checkoutSessionId":SESSION,
            "cart":quote(paypal),"orderCustomer":{"customerId":"customer-1"},"payment":payment,
            "legal":{"policyVersion":"policy-7"}})
    }

    #[test]
    fn a_session_key_needs_a_canonical_uuid() {
        assert_eq!(
            key(SESSION).ok(),
            Some(format!("checkout-session-v1:{SESSION}"))
        );
        for invalid in [SESSION.to_uppercase().as_str(), "not-a-session", ""] {
            assert!(key(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn a_receipt_answers_only_its_own_session_cart_and_customer() {
        let Ok(simulated) = receipt(&placed(false), SESSION, "cart-1", "customer-1", true) else {
            panic!("receipt")
        };
        assert_eq!(simulated["orderId"], "order-1");
        assert_eq!(simulated["totalMinor"], "1000");
        assert!(simulated.get("paymentAttemptId").is_none());
        let Ok(sandbox) = receipt(&placed(true), SESSION, "cart-1", "customer-1", true) else {
            panic!("receipt")
        };
        assert_eq!(sandbox["paymentAttemptId"], "attempt-1");
        for (session, cart, customer) in [
            (
                "3f2b8c1e-5a4d-4e6f-8a9b-000000000000",
                "cart-1",
                "customer-1",
            ),
            (SESSION, "cart-2", "customer-1"),
            (SESSION, "cart-1", "customer-2"),
        ] {
            let refused = receipt(&placed(false), session, cart, customer, true);
            assert_eq!(refused.err().map(|e| e.0), Some(StatusCode::NOT_FOUND));
        }
        let mut charged = placed(false);
        charged["payment"]["realMoneyCharged"] = json!(true);
        assert!(receipt(&charged, SESSION, "cart-1", "customer-1", true).is_err());
    }

    #[test]
    fn the_locked_cart_must_still_be_the_reviewed_customer_cart() {
        let r = review(false);
        assert_eq!(r.session(), SESSION);
        assert!(r.admit_cart(&cart(None), "default", "customer-1").is_ok());
        assert!(r.admit_cart(&cart(None), "other", "customer-1").is_err());
        assert!(
            r.admit_cart(&cart(Some("merchant-1")), "default", "customer-1")
                .is_err()
        );
        let mut language = cart(None);
        language.data.locale = "en-GB".into();
        assert!(r.admit_cart(&language, "default", "customer-1").is_err());
        let foreign = r.admit_cart(&cart(None), "default", "customer-2");
        assert_eq!(foreign.err().map(|e| e.0), Some(StatusCode::FORBIDDEN));
        let mut saved = cart(None);
        saved.data.checkout = Some(CheckoutSelection {
            payment_funding: Some(FundingChoice {
                save: true,
                id: None,
            }),
            ..CheckoutSelection::defaults()
        });
        assert!(r.admit_cart(&saved, "default", "customer-1").is_ok());
        assert!(
            review(true)
                .admit_cart(&saved, "default", "customer-1")
                .is_err()
        );
    }

    #[test]
    fn the_quote_must_match_every_reviewed_value() {
        let r = review(false);
        let policy = json!("policy-7");
        assert!(r.check_quote(&quote(false), &policy, 1000).is_ok());
        assert!(r.check_quote(&quote(false), &policy, 1001).is_err());
        assert!(
            r.check_quote(&quote(false), &json!("policy-8"), 1000)
                .is_err()
        );
        assert!(r.check_quote(&quote(true), &policy, 1000).is_err());
        for (pointer, value) in [
            ("/revision", json!(4)),
            ("/price/currency", json!("USD")),
            ("/checkout/country", json!("AT")),
            ("/selectionNeedsConfirmation", json!(true)),
        ] {
            let mut changed = quote(false);
            if pointer == "/selectionNeedsConfirmation" {
                changed["selectionNeedsConfirmation"] = value;
            } else {
                *changed.pointer_mut(pointer).expect("field") = value;
            }
            assert!(r.check_quote(&changed, &policy, 1000).is_err(), "{pointer}");
        }
        assert!(
            review(true)
                .check_quote(&quote(true), &policy, 1000)
                .is_ok()
        );
        assert!(
            review(true)
                .check_quote(&quote(false), &policy, 1000)
                .is_err()
        );
    }

    #[test]
    fn a_replay_answers_only_the_original_payment_profile() {
        assert!(review(false).check_replay(&placed(false), true).is_ok());
        assert!(review(true).check_replay(&placed(true), true).is_ok());
        assert!(review(true).check_replay(&placed(false), true).is_err());
        assert!(review(false).check_replay(&placed(true), true).is_err());
        let mut amount = placed(false);
        amount["cart"]["price"]["totalMinor"] = json!("1001");
        assert!(review(false).check_replay(&amount, true).is_err());
    }

    #[test]
    fn a_prepared_payment_must_be_the_sandbox_attempt_of_a_sandbox_review() {
        let payment = placed(true)["payment"].clone();
        assert!(review(true).check_prepared_payment(&payment, true).is_ok());
        assert!(
            review(false)
                .check_prepared_payment(&payment, true)
                .is_err()
        );
        assert!(
            review(true)
                .check_prepared_payment(&placed(false)["payment"], true)
                .is_err()
        );
    }
}
