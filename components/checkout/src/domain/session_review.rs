//! The purchase a buyer explicitly reviewed in a checkout session, and the checks that bind the order to exactly that review.
use crate::domain::session_payment;
use serde::Deserialize;
use serde_json::{Value, json};
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

/// What the session service learns about its order; `strict` excludes the contract fixture as a test environment.
pub fn receipt(
    order: &Value,
    session: &str,
    cart: &str,
    customer: &str,
    strict: bool,
) -> Result<Value> {
    let payment = &order["payment"];
    let sandbox_attempt = session_payment::attempt(payment, strict);
    if order["checkoutSessionId"] != session
        || order["cart"]["id"] != cart
        || order["orderCustomer"]["customerId"] != customer
        || !(sandbox_attempt
            || (payment["provider"] == "simulated" && payment["realMoneyCharged"] == false))
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
        "orderId": order["id"],
        "orderNumber": order["orderNumber"],
        "orderState": order["state"],
        "paymentState": order["payment"]["state"],
        "currency": order["cart"]["price"]["currency"],
        "totalMinor": order["cart"]["price"]["totalMinor"],
    });
    if sandbox_attempt {
        result["paymentAttemptId"] = payment["attemptId"].clone();
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
        let method = &q["paymentMethod"];
        let allowed = if self.paypal_sandbox {
            session_payment::provider(method).is_some()
        } else {
            method["mode"] == "simulated"
        };
        if !allowed {
            return Err(conflict(
                "The reviewed payment method is unavailable for this checkout profile",
            ));
        }
        if q["revision"] != self.cart_revision
            || q["price"]["currency"] != self.currency
            || q["checkout"]["country"] != self.country
            || q["paymentMethod"]["id"] != self.payment_method
            || policy != &self.legal_policy_version
            || minor.to_string() != self.total_minor
            || q["selectionNeedsConfirmation"] == true
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
        if self.paypal_sandbox != session_payment::attempt(&order["payment"], strict) {
            return Err(conflict("Review the original checkout payment profile"));
        }
        let total = order["cart"]["price"]["totalMinor"]
            .as_str()
            .and_then(|value| value.parse().ok())
            .ok_or(conflict("Checkout receipt amount unavailable"))?;
        self.check_quote(&order["cart"], &order["legal"]["policyVersion"], total)
    }

    pub fn check_prepared_payment(&self, payment: &Value, strict: bool) -> Result<()> {
        if self.paypal_sandbox && session_payment::attempt(payment, strict) {
            Ok(())
        } else {
            Err(conflict("Use a PayPal Sandbox account for this checkout"))
        }
    }
}
