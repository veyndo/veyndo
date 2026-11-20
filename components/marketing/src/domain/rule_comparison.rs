//! Side-effect-free comparison of two rule conditions on a frozen Flow order input.
use super::rules::Condition;
use serde_json::{Value, json};
use veyndo_core::hash;
use veyndo_shop::types::cart::{Cart, StoredCart};

/// Digest of a condition or snapshot, so a comparison names exactly what it evaluated.
pub fn digest(v: &Value) -> String {
    hash(&v.to_string())
}

/// Evaluates both conditions on one frozen order input; anything incomplete is reported as unavailable, never guessed.
pub fn evaluate(t: &str, d: &Value, before: &Condition, after: &Condition) -> Value {
    let unavailable = |reason| json!({"status":"unavailable","reason":reason});
    if d["event"] != "order.placed" {
        return unavailable("not_order_placed");
    }
    let q = &d["ruleContext"];
    if !q["ruleFacts"].is_object()
        || !q["price"]["totalPrice"].is_number()
        || !q["lineItems"].is_array()
        || !q["ruleFacts"]["currentTime"].is_string()
        || d["orderId"].as_str().is_none_or(str::is_empty)
    {
        return unavailable("incomplete_snapshot");
    }
    let Ok(data) = serde_json::from_value::<Cart>(d["cartData"].clone()) else {
        return unavailable("incomplete_snapshot");
    };
    let cart = StoredCart {
        id: String::new(),
        tenant: t.into(),
        token: String::new(),
        data,
        revision: 0,
        status: "frozen_comparison".into(),
    };
    if !bounded(before, q, 0, &mut 2048) || !bounded(after, q, 0, &mut 2048) {
        return unavailable("required_facts_unavailable");
    }
    // Native interpreter fails closed for missing referenced rules/source facts,
    // including under NOT. Never return raw errors or frozen personal data.
    match (
        before.checked_matches(&cart, q),
        after.checked_matches(&cart, q),
    ) {
        (Ok(a), Ok(b)) => json!({"status":"evaluated","before":a,"after":b,
            "capturedAt":q["ruleFacts"]["currentTime"],
            "snapshotDigest":digest(&json!({"cartData":d["cartData"],"ruleContext":q}))}),
        _ => unavailable("required_facts_unavailable"),
    }
}

// Bound expanded reference work, not just distinct rules: a small shared graph
// can otherwise fan out exponentially. This guard never decides a rule's result.
fn bounded(c: &Condition, q: &Value, depth: usize, remaining: &mut usize) -> bool {
    if depth > 8 || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    match c {
        Condition::And { children } | Condition::Or { children } => children
            .iter()
            .all(|child| bounded(child, q, depth + 1, remaining)),
        Condition::Not { child } => bounded(child, q, depth + 1, remaining),
        Condition::Reference { rule_id } => serde_json::from_value::<Condition>(
            q["ruleFacts"]["rules"][rule_id]["condition"].clone(),
        )
        .is_ok_and(|rule| rule.validate(0).is_ok() && bounded(&rule, q, depth + 1, remaining)),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{Condition, bounded, digest, evaluate};
    use serde_json::{Value, json};
    #[test]
    fn shared_reference_fanout_is_bounded_before_interpretation() {
        let mut q = json!({"ruleFacts":{"rules":{}}});
        for n in 0..3 {
            let child = if n == 2 {
                json!({"type":"alwaysValid"})
            } else {
                json!({"type":"ruleReference","ruleId":format!("r{}",n+1)})
            };
            q["ruleFacts"]["rules"][format!("r{n}")] = json!({"active":true,
                "condition":{"type":"andContainer","children":vec![child;16]}});
        }
        let condition = Condition::Reference {
            rule_id: "r0".into(),
        };
        let mut remaining = 2048;
        assert!(!bounded(&condition, &q, 0, &mut remaining));
        assert_eq!(remaining, 0);
    }
    #[test]
    fn absent_reference_under_negation_never_becomes_a_match() {
        let condition = Condition::Not {
            child: Box::new(Condition::Reference {
                rule_id: "missing".into(),
            }),
        };
        assert!(!bounded(&condition, &json!({}), 0, &mut 2048));
        assert!(bounded(&Condition::Always, &json!({}), 0, &mut 2048));
    }
    fn amount(at_least: f64) -> Condition {
        Condition::Amount {
            operator: ">=".into(),
            amount: at_least,
        }
    }

    fn frozen() -> Value {
        json!({"event":"order.placed","orderId":"order-1",
            "cartData":{"items":[{"id":"p1","quantity":1}],"group":"retail","session":"s","email":null,"company":null,"buyer":null,"order":null},
            "ruleContext":{"ruleFacts":{"currentTime":"2026-01-01T00:00:00Z","rules":{}},"price":{"totalPrice":120.0},"lineItems":[]}})
    }

    #[test]
    fn digests_name_exactly_one_value() {
        assert_eq!(digest(&json!({"a":1})), digest(&json!({"a":1})));
        assert_ne!(digest(&json!({"a":1})), digest(&json!({"a":2})));
    }

    #[test]
    fn both_conditions_are_evaluated_on_the_same_frozen_order() {
        let d = frozen();
        let v = evaluate("shop", &d, &amount(100.), &amount(200.));
        assert_eq!(v["status"], "evaluated");
        assert_eq!(v["before"], true);
        assert_eq!(v["after"], false);
        assert_eq!(v["capturedAt"], "2026-01-01T00:00:00Z");
        assert_eq!(
            v["snapshotDigest"],
            digest(&json!({"cartData":d["cartData"],"ruleContext":d["ruleContext"]}))
        );
    }

    #[test]
    fn incomplete_input_is_unavailable_and_never_guessed() {
        let unavailable = |d: &Value, after: &Condition| {
            let v = evaluate("shop", d, &amount(100.), after);
            assert_eq!(v["status"], "unavailable");
            v["reason"].as_str().map(String::from)
        };
        let mut other = frozen();
        other["event"] = json!("order.paid");
        assert_eq!(
            unavailable(&other, &amount(1.)).as_deref(),
            Some("not_order_placed")
        );
        for key in ["price", "lineItems", "ruleFacts"] {
            let mut d = frozen();
            d["ruleContext"].as_object_mut().unwrap().remove(key);
            assert_eq!(
                unavailable(&d, &amount(1.)).as_deref(),
                Some("incomplete_snapshot"),
                "{key}"
            );
        }
        let mut no_order = frozen();
        no_order["orderId"] = json!("");
        assert_eq!(
            unavailable(&no_order, &amount(1.)).as_deref(),
            Some("incomplete_snapshot")
        );
        let mut no_cart = frozen();
        no_cart["cartData"] = json!({});
        assert_eq!(
            unavailable(&no_cart, &amount(1.)).as_deref(),
            Some("incomplete_snapshot")
        );
        let missing = Condition::Not {
            child: Box::new(Condition::Reference {
                rule_id: "gone".into(),
            }),
        };
        assert_eq!(
            unavailable(&frozen(), &missing).as_deref(),
            Some("required_facts_unavailable")
        );
    }
}
