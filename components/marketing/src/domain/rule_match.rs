//! Evaluate typed rule trees against server-owned cart, customer and event facts.
use super::rules::Condition;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::{StoredCart, selection};
impl Condition {
    pub fn checked_matches(&self, c: &StoredCart, q: &Value) -> Result<bool> {
        self.checked_at(c, q, 0)
    }
    fn checked_at(&self, c: &StoredCart, q: &Value, depth: usize) -> Result<bool> {
        if depth > 8 {
            return Err(bad("Rule references contain a cycle or exceed depth 8"));
        }
        match self {
            Self::Reference { rule_id } => {
                let row = &q["ruleFacts"]["rules"][rule_id];
                if !row["active"].is_boolean() {
                    return Err(bad("Referenced rule unavailable in this tenant"));
                }
                if row["active"] == false {
                    return Err(bad("Referenced rule disabled"));
                }
                let rule: Condition = serde_json::from_value(row["condition"].clone())
                    .map_err(|_| bad("Invalid referenced rule"))?;
                rule.validate(0)?;
                rule.checked_at(c, q, depth + 1)
            }
            Self::Source { name, config } => {
                veyndo_automation::rules::evaluate(name, config, &q["ruleFacts"]).map_err(bad)
            }
            Self::And { children } | Self::Or { children } => {
                let values = children
                    .iter()
                    .map(|v| v.checked_at(c, q, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                Ok(if matches!(self, Self::And { .. }) {
                    values.iter().all(|x| *x)
                } else {
                    values.contains(&true)
                })
            }
            Self::Not { child } => Ok(!child.checked_at(c, q, depth + 1)?),
            _ => Ok(self.matches(c, q)),
        }
    }
    pub fn matches(&self, c: &StoredCart, q: &Value) -> bool {
        match self {
            Self::Reference { .. } | Self::Source { .. } => {
                self.checked_matches(c, q).unwrap_or(false)
            }
            Self::And { children } => children.iter().all(|v| v.matches(c, q)),
            Self::Or { children } => children.iter().any(|v| v.matches(c, q)),
            Self::Not { child } => !child.matches(c, q),
            Self::Always => true,
            Self::Amount { operator, amount } => numeric(
                view::<MatchedQuote>(q).price.total_price.unwrap_or(0.),
                *amount,
                operator,
            ),
            Self::Count { operator, count } => numeric(c.data.items.len() as f64, *count, operator),
            Self::Group { values, operator } => {
                set_matches(std::slice::from_ref(&c.data.group), values, operator)
            }
            Self::Country { values, operator } => {
                set_matches(&[selection(&c.data).country], values, operator)
            }
            Self::Channel { values, operator } => set_matches(
                std::slice::from_ref(&c.data.sales_channel),
                values,
                operator,
            ),
            Self::Product { values, operator } => set_matches(
                &c.data
                    .items
                    .iter()
                    .map(|i| i.id.clone())
                    .collect::<Vec<_>>(),
                values,
                operator,
            ),
            Self::OriginalProduct { values, operator } => {
                view::<MatchedQuote>(q).line_items.is_some_and(|items| {
                    items.iter().map(view::<MatchedItem>).any(|item| {
                        let Some(id) = item.referenced_id.as_deref() else {
                            return false;
                        };
                        item.parent_id
                            .as_deref()
                            .is_some_and(|parent| set_matches(&[parent.into()], values, operator))
                            || set_matches(&[id.into()], values, operator)
                    })
                })
            }
            Self::OrderState { values, operator } => set_matches(
                &view::<MatchedQuote>(q)
                    .order_state
                    .as_deref()
                    .map(String::from)
                    .into_iter()
                    .collect::<Vec<_>>(),
                values,
                operator,
            ),
            Self::PaymentState { values, operator } => set_matches(
                &view::<MatchedQuote>(q)
                    .payment_state
                    .as_deref()
                    .map(String::from)
                    .into_iter()
                    .collect::<Vec<_>>(),
                values,
                operator,
            ),
            Self::DeliveryState { values, operator } => set_matches(
                &view::<MatchedQuote>(q)
                    .delivery_states
                    .map(|v| {
                        v.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                values,
                operator,
            ),
            Self::ShippingMethod { values, operator } => {
                set_matches(&[selection(&c.data).shipping_method_id], values, operator)
            }
            Self::PaymentMethod { values, operator } => {
                set_matches(&[selection(&c.data).payment_method_id], values, operator)
            }
            Self::BillingCountry { values, operator } => {
                let ids = if c.data.customer_id.is_some() {
                    selection(&c.data)
                        .billing_address
                        .map(|a| a.country)
                        .into_iter()
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };
                set_matches(&ids, values, operator)
            }
            Self::CustomerShippingCountry { values, operator } => {
                let country = selection(&c.data).country;
                let ids = if country.is_empty() {
                    vec![]
                } else {
                    vec![country]
                };
                set_matches(&ids, values, operator)
            }
            Self::Email { email, operator } => {
                if c.data.customer_id.is_none() {
                    return operator == "!=";
                }
                let matched = wildcard(c.data.email.as_deref().unwrap_or(""), email);
                veyndo_pricing::verified_kernel::rule_boolean_comparison(
                    matched,
                    false,
                    operator == "=",
                    operator == "!=",
                    false,
                )
            }
            Self::LoggedIn { is_logged_in } => veyndo_pricing::verified_kernel::rule_authenticated(
                c.data.customer_id.is_some(),
                *is_logged_in,
            ),
            Self::Field {
                field,
                operator,
                value,
            } => crate::domain::rule_fields::matches(c, q, field, operator, value),
            Self::EventField {
                path,
                operator,
                value,
            } => {
                let mut item = &q["event"];
                for key in path.split('.') {
                    item = &item[key];
                }
                crate::domain::rule_fields::compare(item, operator, value)
            }
        }
    }
}
fn numeric(a: f64, b: f64, op: &str) -> bool {
    veyndo_automation::rule_comparison::numeric(Some(a), Some(b), op).unwrap_or(false)
}
fn set_matches(actual: &[String], rule: &[String], operator: &str) -> bool {
    veyndo_automation::rule_comparison::uuids(
        Some(&actual.iter().cloned().map(Some).collect::<Vec<_>>()),
        Some(&rule.iter().cloned().map(Some).collect::<Vec<_>>()),
        operator,
    )
    .unwrap_or(false)
}

fn wildcard(value: &str, pattern: &str) -> bool {
    let value = value.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    let (v, p) = (value.as_bytes(), pattern.as_bytes());
    let (mut i, mut j, mut star, mut at) = (0, 0, None, 0);
    while i < v.len() {
        if j < p.len() && p[j] == v[i] {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == b'*' {
            star = Some(j);
            j += 1;
            at = i;
        } else if let Some(s) = star {
            at += 1;
            i = at;
            j = s + 1;
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == b'*' {
        j += 1;
    }
    j == p.len()
}

#[cfg(test)]
mod tests {
    use super::{Condition, wildcard};
    use serde_json::{Value, json};
    use veyndo_shop::types::cart::StoredCart;

    fn cart(extra: Value) -> StoredCart {
        let mut data = json!({"items":[{"id":"p1","quantity":2},{"id":"p2","quantity":3}],"group":"retail","session":"s","email":null,"company":null,"buyer":null,"order":null});
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

    fn condition(v: Value) -> Condition {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn leaf_conditions_read_the_cart_and_the_supplied_facts() {
        let c = cart(json!({"customer_id":"c1","email":"Buyer@Example.test"}));
        let q = json!({"price":{"totalPrice":120.0},"orderState":"open","paymentState":"paid","deliveryStates":["shipped"],
            "lineItems":[{"referencedId":"v1","parentId":"p9"}],"event":{"order":{"number":"A-1"}}});
        for (v, expected) in [
            (json!({"type":"alwaysValid"}), true),
            (
                json!({"type":"cartCartAmount","operator":">=","amount":100.0}),
                true,
            ),
            (
                json!({"type":"cartCartAmount","operator":"<","amount":100.0}),
                false,
            ),
            (
                json!({"type":"cartLineItemCount","operator":"=","count":2.0}),
                true,
            ),
            (json!({"type":"customerGroup","values":["retail"]}), true),
            (
                json!({"type":"customerGroup","values":["retail"],"operator":"!="}),
                false,
            ),
            (json!({"type":"salesChannel","values":["default"]}), true),
            (json!({"type":"lineItemId","values":["p2"]}), true),
            (json!({"type":"lineItemId","values":["p3"]}), false),
            (json!({"type":"cartLineItem","values":["p9"]}), true),
            (json!({"type":"cartLineItem","values":["v1"]}), true),
            (json!({"type":"cartLineItem","values":["p1"]}), false),
            (json!({"type":"orderState","values":["open"]}), true),
            (json!({"type":"paymentState","values":["open"]}), false),
            (json!({"type":"deliveryState","values":["shipped"]}), true),
            (json!({"type":"shippingMethod","values":["pickup"]}), true),
            (json!({"type":"paymentMethod","values":["invoice"]}), false),
            (
                json!({"type":"customerEmail","email":"*@example.test","operator":"="}),
                true,
            ),
            (
                json!({"type":"customerEmail","email":"*@other.test","operator":"!="}),
                true,
            ),
            (json!({"type":"customerLoggedIn","isLoggedIn":true}), true),
            (json!({"type":"customerLoggedIn","isLoggedIn":false}), false),
            (
                json!({"type":"contextField","field":"cart.quantity","operator":">","value":4}),
                true,
            ),
            (
                json!({"type":"eventField","path":"order.number","operator":"=","value":"a-1"}),
                true,
            ),
            (
                json!({"type":"eventField","path":"order.missing","operator":"=","value":"a-1"}),
                false,
            ),
        ] {
            assert_eq!(condition(v.clone()).matches(&c, &q), expected, "{v}");
        }
        let guest = cart(json!({"email":"buyer@example.test"}));
        assert!(
            !condition(json!({"type":"customerEmail","email":"*","operator":"="}))
                .matches(&guest, &q)
        );
        assert!(
            condition(json!({"type":"customerEmail","email":"*","operator":"!="}))
                .matches(&guest, &q)
        );
    }

    #[test]
    fn containers_combine_and_references_resolve_only_active_rules_of_the_snapshot() {
        let c = cart(json!({}));
        let q = json!({"price":{"totalPrice":50.0},"ruleFacts":{"rules":{
            "big":{"active":true,"condition":{"type":"cartCartAmount","operator":">=","amount":100.0}},
            "small":{"active":true,"condition":{"type":"cartCartAmount","operator":"<","amount":100.0}},
            "off":{"active":false,"condition":{"type":"alwaysValid"}},
            "loop":{"active":true,"condition":{"type":"ruleReference","ruleId":"loop"}}}}});
        let reference = |id: &str| json!({"type":"ruleReference","ruleId":id});
        assert_eq!(
            condition(reference("small")).checked_matches(&c, &q).ok(),
            Some(true)
        );
        assert_eq!(
            condition(reference("big")).checked_matches(&c, &q).ok(),
            Some(false)
        );
        for id in ["off", "missing", "loop"] {
            assert!(
                condition(reference(id)).checked_matches(&c, &q).is_err(),
                "{id}"
            );
            assert!(!condition(reference(id)).matches(&c, &q), "{id}");
        }
        let negated_missing =
            condition(json!({"type":"notContainer","child":reference("missing")}));
        assert!(negated_missing.checked_matches(&c, &q).is_err());
        let and = condition(
            json!({"type":"andContainer","children":[reference("small"),{"type":"alwaysValid"}]}),
        );
        assert_eq!(and.checked_matches(&c, &q).ok(), Some(true));
        assert!(and.matches(&c, &q));
        let or = condition(
            json!({"type":"orContainer","children":[reference("big"),reference("small")]}),
        );
        assert_eq!(or.checked_matches(&c, &q).ok(), Some(true));
        let not = condition(json!({"type":"notContainer","child":reference("small")}));
        assert_eq!(not.checked_matches(&c, &q).ok(), Some(false));
        assert!(!not.matches(&c, &q));
        let failing_or = condition(
            json!({"type":"orContainer","children":[{"type":"alwaysValid"},reference("missing")]}),
        );
        assert!(failing_or.checked_matches(&c, &q).is_err());
    }

    #[test]
    fn wildcards_match_case_insensitively() {
        assert!(wildcard("Buyer@Example.test", "*@example.TEST"));
        assert!(wildcard("buyer@example.test", "buyer@*"));
        assert!(wildcard("buyer@example.test", "b*@*.test"));
        assert!(wildcard("", "*"));
        assert!(!wildcard("buyer@example.test", "*@other.test"));
        assert!(!wildcard("buyer@example.test", "buyer"));
        assert!(!wildcard("", "a"));
    }
}

/// The evaluated quote facts native conditions compare.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MatchedQuote {
    #[serde(deserialize_with = "lenient")]
    price: MatchedPrice,
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    order_state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    payment_state: Option<String>,
    #[serde(deserialize_with = "lenient")]
    delivery_states: Option<Vec<Value>>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MatchedPrice {
    #[serde(deserialize_with = "lenient")]
    total_price: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MatchedItem {
    #[serde(deserialize_with = "lenient")]
    referenced_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    parent_id: Option<String>,
}
