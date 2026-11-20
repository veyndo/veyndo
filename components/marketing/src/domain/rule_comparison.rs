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
    use super::{Condition, bounded};
    use serde_json::json;
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
}
