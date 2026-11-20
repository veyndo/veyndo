//! Typed rule fields share the original comparison operators and server-derived checkout/event facts.
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
use veyndo_shop::types::cart::{StoredCart, selection};
pub const FIELDS: &[&str] = &[
    "customer.email",
    "customer.company",
    "customer.group",
    "customer.loggedIn",
    "cart.quantity",
    "cart.shippingMethod",
    "cart.paymentMethod",
    "cart.billingCountry",
    "cart.shippingCountry",
    "cart.salesChannel",
    "order.state",
    "payment.state",
    "event.kind",
    "event.title",
    "event.sourceKind",
];
pub fn validate(field: &str, operator: &str, value: &Value) -> Result<()> {
    if !FIELDS.contains(&field) {
        return Err(bad("Unsupported rule field"));
    }
    validate_value(operator, value)
}
pub fn validate_value(operator: &str, value: &Value) -> Result<()> {
    if !["=", "!=", ">", ">=", "<", "<=", "empty", "contains"].contains(&operator)
        || !value.is_string() && !value.is_boolean() && !value.is_number()
    {
        return Err(bad("Invalid field comparison"));
    }
    if value.is_boolean() && !["=", "!="].contains(&operator)
        || value.is_string() && !["=", "!=", "empty", "contains"].contains(&operator)
    {
        return Err(bad("Operator incompatible with field value"));
    }
    if value.to_string().len() > 500 {
        return Err(bad("Rule value exceeds limit"));
    }
    Ok(())
}
pub fn matches(c: &StoredCart, q: &Value, field: &str, operator: &str, value: &Value) -> bool {
    let selection = selection(&c.data);
    let actual = match field {
        "customer.email" => json!(c.data.email),
        "customer.company" => json!(c.data.company),
        "customer.group" => json!(c.data.group),
        "customer.loggedIn" => json!(c.data.customer_id.is_some()),
        "cart.quantity" => json!(
            c.data
                .items
                .iter()
                .map(|i| u64::from(i.quantity))
                .sum::<u64>()
        ),
        "cart.shippingMethod" => json!(selection.shipping_method_id),
        "cart.paymentMethod" => json!(selection.payment_method_id),
        "cart.billingCountry" => json!(selection.billing_address.map(|a| a.country)),
        "cart.shippingCountry" => json!(selection.country),
        "cart.salesChannel" => json!(c.data.sales_channel),
        "order.state" => q["orderState"].clone(),
        "payment.state" => q["paymentState"].clone(),
        "event.kind" => q["eventKind"].clone(),
        "event.title" => q["event"]["title"].clone(),
        "event.sourceKind" => q["event"]["sourceKind"].clone(),
        _ => return false,
    };
    compare(&actual, operator, value)
}
pub fn compare(item: &Value, operator: &str, rule: &Value) -> bool {
    if rule.is_number() {
        return veyndo_automation::rule_comparison::numeric(item.as_f64(), rule.as_f64(), operator)
            .unwrap_or(false);
    }
    if rule.is_boolean() {
        return match operator {
            "=" => item.as_bool() == rule.as_bool(),
            "!=" => item.as_bool().is_some() && item.as_bool() != rule.as_bool(),
            _ => false,
        };
    }
    if operator == "contains" {
        return item
            .as_str()
            .zip(rule.as_str())
            .is_some_and(|(a, b)| a.to_lowercase().contains(&b.to_lowercase()));
    }
    veyndo_automation::rule_comparison::string(item.as_str(), rule.as_str().unwrap_or(""), operator)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{compare, matches, validate, validate_value};
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

    #[test]
    fn comparisons_need_a_known_field_and_an_operator_that_fits_the_value() {
        assert!(validate("customer.group", "=", &json!("retail")).is_ok());
        assert!(validate("customer.password", "=", &json!("x")).is_err());
        assert!(validate_value(">=", &json!(3)).is_ok());
        assert!(validate_value("contains", &json!("a")).is_ok());
        assert!(validate_value("!=", &json!(true)).is_ok());
        assert!(validate_value(">", &json!(true)).is_err());
        assert!(validate_value(">", &json!("a")).is_err());
        assert!(validate_value("~", &json!("a")).is_err());
        assert!(validate_value("=", &json!(["a"])).is_err());
        assert!(validate_value("=", &Value::Null).is_err());
        assert!(validate_value("=", &json!("x".repeat(500))).is_err());
    }

    #[test]
    fn values_compare_by_their_type() {
        assert!(compare(&json!(5), ">", &json!(3)));
        assert!(!compare(&json!(5), "<", &json!(3)));
        assert!(!compare(&json!("5"), ">", &json!(3)));
        assert!(compare(&json!(true), "=", &json!(true)));
        assert!(compare(&json!(false), "!=", &json!(true)));
        assert!(!compare(&Value::Null, "!=", &json!(true)));
        assert!(!compare(&json!(true), ">", &json!(true)));
        assert!(compare(&json!("Hello World"), "contains", &json!("WORLD")));
        assert!(!compare(&Value::Null, "contains", &json!("x")));
        assert!(compare(&json!("Retail"), "=", &json!("retail")));
        assert!(compare(&json!("retail"), "!=", &json!("business")));
        assert!(compare(&Value::Null, "empty", &json!("")));
        assert!(!compare(&json!("x"), "empty", &json!("")));
    }

    #[test]
    fn fields_read_server_owned_cart_and_event_facts() {
        let c =
            cart(json!({"email":"buyer@example.test","customer_id":"c1","sales_channel":"outlet"}));
        let q = json!({"orderState":"open","paymentState":"paid","eventKind":"order.placed","event":{"title":"Big order","sourceKind":"shop"}});
        for (field, operator, value) in [
            ("customer.email", "contains", json!("@example.test")),
            ("customer.company", "empty", json!("")),
            ("customer.group", "=", json!("retail")),
            ("customer.loggedIn", "=", json!(true)),
            ("cart.quantity", "=", json!(5)),
            ("cart.shippingMethod", "=", json!("pickup")),
            ("cart.paymentMethod", "=", json!("demo-card")),
            ("cart.billingCountry", "empty", json!("")),
            ("cart.salesChannel", "=", json!("outlet")),
            ("order.state", "=", json!("open")),
            ("payment.state", "!=", json!("open")),
            ("event.kind", "=", json!("order.placed")),
            ("event.title", "contains", json!("big")),
            ("event.sourceKind", "=", json!("shop")),
        ] {
            assert!(matches(&c, &q, field, operator, &value), "{field}");
        }
        assert!(!matches(&c, &q, "customer.password", "=", &json!("x")));
        assert!(!matches(
            &cart(json!({})),
            &q,
            "customer.loggedIn",
            "=",
            &json!(true)
        ));
    }
}
