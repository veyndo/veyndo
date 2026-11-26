//! Typed reads of an app action input schema: the declared `properties` the host binds and filters by.
//! The rest of the schema is app-declared JSON Schema that the native validator checks.
use serde::Deserialize;
use serde_json::{Map, Value};
use veyndo_core::document::{lenient, view};

/// The declared input properties of an action input schema, by name; empty when the schema declares none.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct InputProperties {
    #[serde(deserialize_with = "lenient")]
    pub properties: Map<String, Value>,
}

/// The `type` keyword of one declared property; `null` when it has none.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PropertyType {
    #[serde(rename = "type")]
    kind: Value,
}

impl InputProperties {
    /// Reads the declared properties of `schema`.
    pub fn of(schema: &Value) -> Self {
        view(schema)
    }

    /// Whether the schema declares the input `name`.
    pub fn declares(&self, name: &str) -> bool {
        self.properties.contains_key(name)
    }

    /// The declared JSON type of input `name`, or `null` when it is undeclared or untyped.
    pub fn kind(&self, name: &str) -> Value {
        self.properties
            .get(name)
            .map(|property| view::<PropertyType>(property).kind)
            .unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::InputProperties;
    use serde_json::{Value, json};

    #[test]
    fn reads_like_indexing_the_schema() {
        let schemas = [
            json!({"properties":{"a":{"type":"string"},"b":{},"c":7}}),
            json!({"properties":[1]}),
            json!("schema"),
            Value::Null,
        ];
        for schema in schemas {
            let typed = InputProperties::of(&schema);
            for name in ["a", "b", "c", "missing"] {
                assert_eq!(
                    typed.declares(name),
                    schema["properties"].get(name).is_some()
                );
                assert_eq!(typed.kind(name), schema["properties"][name]["type"]);
            }
        }
    }
}
