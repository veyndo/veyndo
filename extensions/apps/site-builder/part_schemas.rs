//! Kit part configurations checked against the JSON Schemas generated from the frontend part contracts.
//!
//! `part-schemas.json` is written by `frontend/tests/unit/site-parts.test.tsx`
//! (`WRITE_PART_SCHEMAS=1`) and that test fails on drift. This validator supports
//! exactly the keywords the contracts emit; a test rejects any other keyword.
use serde_json::{Map, Value};
use std::sync::OnceLock;

/// The placeable parts: id → `{version, config}` JSON Schema.
pub(crate) fn catalogue() -> &'static Map<String, Value> {
    static CATALOGUE: OnceLock<Map<String, Value>> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        serde_json::from_str(include_str!("part-schemas.json"))
            .expect("generated part schemas are valid JSON")
    })
}

/// A failed check: the field path (from the schema, never from input) and the rule.
pub(crate) type Violation = (String, String);

/// Validates one placed part and returns the product IDs its configuration names.
pub(crate) fn validate(id: &str, version: u64, config: &Value) -> Result<Vec<String>, Violation> {
    let Some(entry) = catalogue().get(id) else {
        let ids = catalogue().keys().cloned().collect::<Vec<_>>().join(", ");
        return Err(("part.id".into(), format!("use a placeable part: {ids}")));
    };
    let expected = entry["version"].as_u64().unwrap_or(0);
    if version != expected {
        return Err(("part.version".into(), format!("use version {expected}")));
    }
    let mut products = Vec::new();
    check(&entry["config"], config, "part.config", &mut products)?;
    Ok(products)
}

fn fail(path: &str, rule: &str) -> Violation {
    (path.to_string(), rule.to_string())
}

fn check(
    schema: &Value,
    value: &Value,
    path: &str,
    products: &mut Vec<String>,
) -> Result<(), Violation> {
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        let names = options
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(fail(path, &format!("use one of {names}")));
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let object = value
                .as_object()
                .ok_or_else(|| fail(path, "use an object"))?;
            let empty = Map::new();
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            if object.keys().any(|key| !properties.contains_key(key)) {
                let names = properties.keys().cloned().collect::<Vec<_>>().join(", ");
                return Err(fail(path, &format!("use only the fields {names}")));
            }
            for required in schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = required.as_str().unwrap_or_default();
                if !object.contains_key(name) {
                    return Err(fail(&format!("{path}.{name}"), "required"));
                }
            }
            for (name, spec) in properties {
                if let Some(field) = object.get(name) {
                    check(spec, field, &format!("{path}.{name}"), products)?;
                }
            }
        }
        Some("string") => {
            let text = value.as_str().ok_or_else(|| fail(path, "use a string"))?;
            // Zod counts UTF-16 code units; match it exactly.
            let length = text.encode_utf16().count() as u64;
            let min = schema.get("minLength").and_then(Value::as_u64).unwrap_or(0);
            let max = schema
                .get("maxLength")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX);
            if length < min || length > max {
                return Err(fail(path, &format!("use {min}–{max} characters")));
            }
            if schema.get("format").and_then(Value::as_str) == Some("product-id") {
                products.push(text.to_string());
            }
        }
        Some(kind @ ("number" | "integer")) => {
            let number = value
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| fail(path, "use a number"))?;
            if kind == "integer" && !(value.is_i64() || value.is_u64()) {
                return Err(fail(path, "use a whole number"));
            }
            let min = schema
                .get("minimum")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MIN);
            let max = schema
                .get("maximum")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            if number < min || number > max {
                return Err(fail(path, &format!("use a value from {min} to {max}")));
            }
        }
        Some("boolean") => {
            if !value.is_boolean() {
                return Err(fail(path, "use true or false"));
            }
        }
        _ => return Err(fail(path, "unsupported part schema")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SUPPORTED: &[&str] = &[
        "$schema",
        "type",
        "properties",
        "required",
        "enum",
        "default",
        "minLength",
        "maxLength",
        "format",
        "minimum",
        "maximum",
    ];

    fn keywords(schema: &Value) {
        let object = schema.as_object().expect("schema object");
        for (key, value) in object {
            assert!(
                SUPPORTED.contains(&key.as_str()),
                "unsupported keyword {key}"
            );
            if key == "properties" {
                value.as_object().unwrap().values().for_each(keywords);
            }
        }
    }

    #[test]
    fn generated_schemas_use_only_supported_keywords() {
        assert!(catalogue().contains_key("product-card"));
        for entry in catalogue().values() {
            assert!(entry["version"].as_u64().is_some());
            keywords(&entry["config"]);
        }
    }

    #[test]
    fn configurations_are_checked_against_the_contract() {
        assert_eq!(
            validate(
                "product-card",
                1,
                &json!({"productId": "bag", "priceKind": "net"})
            ),
            Ok(vec!["bag".to_string()])
        );
        let rejected = [
            ("product-card", 1, json!({})),
            ("product-card", 1, json!({"productId": ""})),
            (
                "product-card",
                1,
                json!({"productId": "bag", "priceKind": "free"}),
            ),
            (
                "product-card",
                1,
                json!({"productId": "bag", "script": "x"}),
            ),
            ("product-card", 1, json!({"productId": 7})),
            ("product-card", 1, json!("bag")),
            ("product-card", 2, json!({"productId": "bag"})),
            ("cart-summary", 1, json!({})),
        ];
        for (id, version, config) in rejected {
            assert!(validate(id, version, &config).is_err(), "{id} {config}");
        }
        let (path, rule) =
            validate("product-card", 1, &json!({"productId": "bag", "<b>": 1})).unwrap_err();
        assert_eq!(path, "part.config");
        assert!(!rule.contains("<b>"), "errors never echo input");
        let long = "x".repeat(201);
        assert!(validate("product-summary", 1, &json!({"productId": long})).is_err());
    }
}
