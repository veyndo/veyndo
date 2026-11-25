//! Provider wire adaptation for strict fixed-object schemas; dynamic JSON is encoded only on the model wire and restored before domain validation.
//! JSON Schema documents are open by design: every keyword and property name comes from the caller.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
/// The JSON Schema keywords of a node, as strict-schema adaptation reads them.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SchemaKeywords {
    #[serde(rename = "type")]
    kind: Value,
    properties: Value,
    required: Value,
}

/// The `type` keyword of a node when it is a string.
#[derive(Deserialize, Default)]
#[serde(default)]
struct SchemaType {
    #[serde(rename = "type", deserialize_with = "lenient")]
    kind: Option<String>,
}

/// The keywords strict adaptation sets on an object node: every property required, none additional.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StrictObject {
    required: Vec<String>,
    additional_properties: bool,
}

pub(super) fn strict_schema(mut v: Value) -> Value {
    // OpenAI's strict array subset excludes uniqueItems. Preserve the domain
    // schema and reject duplicates during restoration before any proposal exists.
    if let Some(object) = v.as_object_mut() {
        object.remove("uniqueItems");
    }
    let node: SchemaKeywords = view(&v);
    if v.is_object()
        && ((node.kind == "object" && !node.properties.is_object())
            || (v.get("type").is_none() && v.get("anyOf").is_none() && v.get("enum").is_none()))
    {
        return json!({"type":"string","description":"JSON-encoded value. Encode the entire object/value as a JSON string; the host restores and validates it."});
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(xs) = v.get_mut(key).and_then(Value::as_array_mut) {
            for x in xs {
                *x = strict_schema(x.clone());
            }
        }
    }
    if let Some(items) = v.get_mut("items") {
        *items = strict_schema(items.clone());
    }
    if node.kind == "object" {
        let required = node.required.as_array().cloned().unwrap_or_default();
        // An object node reaching this point has object `properties`; anything else returned above.
        let Some(properties) = v.get_mut("properties").and_then(Value::as_object_mut) else {
            unreachable!("object schema node without properties is JSON-encoded above")
        };
        for (name, value) in properties.iter_mut() {
            *value = strict_schema(value.clone());
            if !required.contains(&json!(name)) {
                *value = json!({"anyOf":[value.clone(),{"type":"null"}]});
            }
        }
        let names = properties.keys().cloned().collect::<Vec<_>>();
        veyndo_core::document::merge(
            &mut v,
            StrictObject {
                required: names,
                additional_properties: false,
            },
        )
        .expect("an object schema node is an object");
    }
    v
}
/// The JSON fields `restore` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RestoreInput {
    r#type: Value,
    properties: Value,
    any_of: Value,
    required: Value,
    #[serde(deserialize_with = "lenient")]
    unique_items: Option<bool>,
}

pub(super) fn restore(schema: &Value, mut value: Value) -> Result<Value, String> {
    let input: RestoreInput = view(schema);
    if value.is_null() {
        return Ok(value);
    }
    if (input.r#type == "object" && !input.properties.is_object())
        || (schema.get("type").is_none()
            && schema.get("anyOf").is_none()
            && schema.get("enum").is_none())
    {
        if let Some(s) = value.as_str() {
            if s.len() > 65536 {
                return Err("Model JSON field exceeds 64 KiB".into());
            }
            value = serde_json::from_str(s).map_err(|_| "Model returned invalid encoded JSON")?;
        }
        return Ok(value);
    }
    if let Some(branches) = input.any_of.as_array() {
        for branch in branches {
            let kind = view::<SchemaType>(branch).kind;
            let t = kind.as_deref().unwrap_or("");
            if (t == "object" && (value.is_object() || value.is_string()))
                || (t == "array" && value.is_array())
                || (t == "string" && value.is_string())
            {
                return restore(branch, value);
            }
        }
    }
    if let Some(properties) = input.properties.as_object()
        && let Some(object) = value.as_object_mut()
    {
        for (key, shape) in properties {
            // Strict providers require every property and encode omitted optional
            // fields as null. Undo only that wire adaptation: a required null or
            // an explicitly nullable/dynamic domain value must still be validated.
            let required = input
                .required
                .as_array()
                .is_some_and(|keys| keys.iter().any(|name| name.as_str() == Some(key)));
            if !required
                && view::<SchemaType>(shape)
                    .kind
                    .is_some_and(|kind| kind != "null")
                && object.get(key).is_some_and(Value::is_null)
            {
                object.remove(key);
                continue;
            }
            if let Some(v) = object.get_mut(key) {
                *v = restore(shape, v.clone())?;
            }
        }
    }
    if let Some(items) = schema.get("items")
        && let Some(array) = value.as_array_mut()
    {
        for v in array.iter_mut() {
            *v = restore(items, v.clone())?;
        }
        if input.unique_items == Some(true) {
            let mut seen = std::collections::HashSet::new();
            for item in array {
                if !seen.insert(item.to_string()) {
                    return Err("Model returned duplicate items in a unique selection".into());
                }
            }
        }
    }
    Ok(value)
}
#[cfg(test)]
/// The untyped adaptation this module replaced, kept as the oracle for the JSON equivalence test.
mod legacy {
    use serde_json::{Value, json};
    pub(super) fn strict_schema(mut v: Value) -> Value {
        // OpenAI's strict array subset excludes uniqueItems. Preserve the domain
        // schema and reject duplicates during restoration before any proposal exists.
        if let Some(object) = v.as_object_mut() {
            object.remove("uniqueItems");
        }
        if v.is_object()
            && ((v["type"] == "object" && !v["properties"].is_object())
                || (v.get("type").is_none() && v.get("anyOf").is_none() && v.get("enum").is_none()))
        {
            return json!({"type":"string","description":"JSON-encoded value. Encode the entire object/value as a JSON string; the host restores and validates it."});
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(xs) = v.get_mut(key).and_then(Value::as_array_mut) {
                for x in xs {
                    *x = strict_schema(x.clone());
                }
            }
        }
        if let Some(items) = v.get_mut("items") {
            *items = strict_schema(items.clone());
        }
        if v["type"] == "object" {
            let required = v["required"].as_array().cloned().unwrap_or_default();
            let properties = v["properties"].as_object_mut().unwrap();
            for (name, value) in properties.iter_mut() {
                *value = strict_schema(value.clone());
                if !required.contains(&json!(name)) {
                    *value = json!({"anyOf":[value.clone(),{"type":"null"}]});
                }
            }
            let names = properties.keys().cloned().collect::<Vec<_>>();
            v["required"] = json!(names);
            v["additionalProperties"] = json!(false);
        }
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unique_product_selection_is_enforced_by_host_not_provider_schema() {
        let schema = json!({"type":"object","properties":{
            "translation_products":{"type":["array","null"],"minItems":1,
                "maxItems":1000,"uniqueItems":true,"items":{"type":"string"}}
        }});
        let wire = strict_schema(schema.clone());
        let selection = &wire["properties"]["translation_products"]["anyOf"][0];
        assert!(selection.get("uniqueItems").is_none());
        assert_eq!(selection["maxItems"], 1000);
        assert!(restore(&schema, json!({"translation_products":["a","a"]})).is_err());
        let distinct = json!({"translation_products":["a","b"]});
        assert_eq!(restore(&schema, distinct.clone()).unwrap(), distinct);
        assert_eq!(
            restore(&schema, json!({"translation_products":null})).unwrap(),
            json!({"translation_products":null})
        );
        assert_eq!(
            schema["properties"]["translation_products"]["uniqueItems"],
            true
        );
    }
    #[test]
    fn wire_only_optional_nulls_restore_absence_without_clearing_domain_values() {
        let schema = json!({"type":"object","required":["changes"],"properties":{
            "changes":{"type":"array","items":{"type":"object"}},
            "operations":{"type":"array","items":{"type":"object"}},
            "manifest":{"type":"object","required":[],"properties":{
                "lifecycles":{"type":"array","items":{"type":"object"}},
                "caption":{"anyOf":[{"type":"string"},{"type":"null"}]},
                "arbitrary":{}
            }}
        }});
        let restored = restore(
            &schema,
            json!({"changes":[],"operations":null,
            "manifest":{"lifecycles":null,"caption":null,"arbitrary":null}}),
        )
        .unwrap();
        assert!(restored.get("operations").is_none());
        assert!(restored["manifest"].get("lifecycles").is_none());
        assert_eq!(restored["manifest"].get("caption"), Some(&Value::Null));
        assert_eq!(restored["manifest"].get("arbitrary"), Some(&Value::Null));
        // Invalid required values remain visible to the ordinary domain validator.
        assert_eq!(
            restore(&schema, json!({"changes":null})).unwrap()["changes"],
            Value::Null
        );
    }
    #[test]
    fn dynamic_fields_roundtrip_and_invalid_wire_json_fails_closed() {
        let schema = json!({"type":"object","properties":{"fields":{"type":"object"},"branches":{"type":"array","items":{"anyOf":[{"type":"object","properties":{"value":{}}},{"type":"null"}]}}}});
        let wire = strict_schema(schema.clone());
        assert_eq!(wire["properties"]["fields"]["anyOf"][0]["type"], "string");
        let value=restore(&schema,json!({"fields":"{\"money\":{\"minor\":125,\"currency\":\"USD\",\"scale\":2}}","branches":[{"value":"[1,true]"}]})).unwrap();
        assert_eq!(value["fields"]["money"]["minor"], 125);
        assert_eq!(value["branches"][0]["value"], json!([1, true]));
        assert!(restore(&schema, json!({"fields":"invalid JSON"})).is_err());
    }
}
#[cfg(test)]
mod equivalence {
    use serde_json::json;

    #[test]
    fn typed_strict_schema_is_unchanged() {
        let schemas = [
            json!({"type":"object","required":["a"],"properties":{"a":{"type":"string"},"b":{"type":"array","uniqueItems":true,"items":{"type":"object","properties":{"c":{}}}},"d":{"anyOf":[{"type":"object"},{"type":"null"}]},"e":{"enum":[1,2]}}}),
            json!({"type":"object","required":"a","properties":{}}),
            json!({"type":"object"}),
            json!({"oneOf":[{"type":"integer"},{"type":"object","properties":{"x":{"type":"number"}}}]}),
            json!({"items":{"type":"string"}}),
            json!(true),
            json!([{"type":"object"}]),
        ];
        for schema in schemas {
            assert_eq!(
                super::strict_schema(schema.clone()),
                super::legacy::strict_schema(schema)
            );
        }
        let studio: serde_json::Value =
            serde_json::from_str(include_str!("../../../../fixtures/app-studio-schema.json"))
                .unwrap();
        assert_eq!(
            super::strict_schema(studio.clone()),
            super::legacy::strict_schema(studio)
        );
    }
}
