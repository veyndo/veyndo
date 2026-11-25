//! Required contact data for explicit registered marketing segments; shared by protocol review and locked native checkout.
use crate::types::cart::StoredCart;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};
/// The buyer contact and marketing consent fields the check reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Buyer {
    consent: Consents,
    #[serde(deserialize_with = "lenient")]
    email: Option<String>,
    #[serde(deserialize_with = "lenient")]
    phone_number: Option<String>,
}

/// The buyer's consent decisions by consent ID.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Consents {
    #[serde(rename = "dev.ucp.consent.marketing", deserialize_with = "lenient")]
    marketing: Marketing,
}

/// The marketing consent with its per-segment decisions.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Marketing {
    #[serde(deserialize_with = "lenient")]
    granted: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    segments: HashMap<String, Value>,
}

/// One segment's decision.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Segment {
    #[serde(deserialize_with = "lenient")]
    granted: Option<bool>,
}

/// Whether a granted marketing segment (or the whole consent) lacks the contact it needs.
pub fn missing_contact(buyer: &Value) -> bool {
    missing(&view(buyer))
}

fn missing(buyer: &Buyer) -> bool {
    let marketing = &buyer.consent.marketing;
    [
        ("dev.ucp.consent.marketing.email", &buyer.email),
        ("dev.ucp.consent.marketing.sms", &buyer.phone_number),
    ]
    .iter()
    .any(|(segment, contact)| {
        let granted = marketing
            .segments
            .get(*segment)
            .and_then(|s| view::<Segment>(s).granted)
            .unwrap_or(marketing.granted == Some(true));
        granted && contact.as_deref().is_none_or(|s| s.trim().is_empty())
    })
}
/// A buyer consent decision.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ConsentReceipt {
    #[serde(deserialize_with = "lenient")]
    decided: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    choices: ConsentChoices,
}

/// The consent choices of a buyer.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ConsentChoices {
    #[serde(deserialize_with = "lenient")]
    marketing: Option<bool>,
}

pub fn check(c: &StoredCart, receipt: &Value) -> Result<()> {
    let receipt: ConsentReceipt = view(receipt);
    if receipt.decided != Some(true) || receipt.choices.marketing != Some(true) {
        return Ok(());
    }
    let mut buyer: Buyer = view(&c.data.buyer.clone().unwrap_or(json!({})));
    if let Some(email) = c.data.email.as_ref().filter(|e| !e.is_empty()) {
        buyer.email = Some(email.clone());
    }
    if let Some(address) = c
        .data
        .checkout
        .as_ref()
        .and_then(|s| s.address.as_ref())
        .filter(|a| !a.phone_number.is_empty())
    {
        buyer.phone_number = Some(address.phone_number.clone());
    }
    if missing(&buyer) {
        return Err(bad(
            "Consent contact data missing; enter the agreed email or phone, or withdraw marketing consent",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check, missing_contact};
    use crate::types::cart::StoredCart;
    use serde_json::{Value, json};

    fn cart(extra: Value) -> StoredCart {
        let mut data = json!({"items":[],"group":"retail","session":"s","email":null,"company":null,"buyer":null,"order":null});
        for (key, value) in extra.as_object().unwrap() {
            data[key] = value.clone();
        }
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(data).unwrap(),
            revision: 1,
            status: "open".into(),
        }
    }

    #[test]
    fn a_granted_segment_needs_its_own_contact() {
        let consent = |segments: Value| json!({"dev.ucp.consent.marketing":{"granted":false,"segments":segments}});
        let email = consent(json!({"dev.ucp.consent.marketing.email":{"granted":true}}));
        assert!(missing_contact(&json!({"consent":email})));
        assert!(missing_contact(&json!({"consent":email,"email":"  "})));
        assert!(!missing_contact(
            &json!({"consent":email,"email":"buyer@example.test"})
        ));
        let sms = consent(json!({"dev.ucp.consent.marketing.sms":{"granted":true}}));
        assert!(missing_contact(
            &json!({"consent":sms,"email":"buyer@example.test"})
        ));
        assert!(!missing_contact(
            &json!({"consent":sms,"phone_number":"+49 30 1"})
        ));
        let all = json!({"dev.ucp.consent.marketing":{"granted":true}});
        assert!(missing_contact(
            &json!({"consent":all,"email":"buyer@example.test"})
        ));
        assert!(!missing_contact(
            &json!({"consent":all,"email":"buyer@example.test","phone_number":"1"})
        ));
        assert!(!missing_contact(&json!({})));
    }

    #[test]
    fn a_marketing_receipt_needs_the_cart_contact_of_each_agreed_segment() {
        let consent = json!({"dev.ucp.consent.marketing":{"segments":{"dev.ucp.consent.marketing.email":{"granted":true}}}});
        let agreed = json!({"decided":true,"choices":{"marketing":true}});
        let without = cart(json!({"buyer":{"consent":consent}}));
        assert!(check(&without, &agreed).is_err());
        assert!(
            check(
                &without,
                &json!({"decided":true,"choices":{"marketing":false}})
            )
            .is_ok()
        );
        assert!(
            check(
                &without,
                &json!({"decided":false,"choices":{"marketing":true}})
            )
            .is_ok()
        );
        let with = cart(json!({"buyer":{"consent":consent},"email":"buyer@example.test"}));
        assert!(check(&with, &agreed).is_ok());
        assert!(check(&cart(json!({})), &agreed).is_ok());
    }
}
