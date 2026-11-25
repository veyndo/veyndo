//! Typed reads and key-preserving writes for model-facing manifest drafts and their JSON Schema.
//! A draft stays open JSON until native `Manifest` validation: the host reads named fields through
//! serde views and patches only those members, so unknown model output survives unchanged.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use veyndo_core::{Result, bad};

/// Field deserializer that keeps a present `null` apart from an absent key, as `Value::get` does.
pub fn present<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// A draft element read only for its `name`; `null` when the element has none.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Named {
    pub name: Value,
}

/// A draft element read only for its `id`; `null` when the element has none.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Identified {
    pub id: Value,
}

/// The elements of a draft list, each read as `T` (its default for another shape); anything but a list is empty.
pub fn elements<T: serde::de::DeserializeOwned + Default>(list: &Value) -> Vec<T> {
    list.as_array()
        .map(|items| items.iter().map(veyndo_core::document::view).collect())
        .unwrap_or_default()
}

/// The draft as an object; a null draft becomes an empty object, exactly as assigning through JSON
/// indexing did. Any other shape is an error where indexing would have panicked.
pub fn object(doc: &mut Value) -> Result<&mut Map<String, Value>> {
    if doc.is_null() {
        *doc = Value::Object(Map::new());
    }
    doc.as_object_mut()
        .ok_or_else(|| bad("Invalid app definition document"))
}

/// The member `key`, inserted as `null` when missing: the slot JSON indexing navigates through.
pub fn member<'a>(doc: &'a mut Value, key: &str) -> Result<&'a mut Value> {
    Ok(object(doc)?.entry(key).or_insert(Value::Null))
}

/// Writes the serialized fields of `update` into the draft and keeps every other member.
pub fn write(doc: &mut Value, update: impl Serialize) -> Result<()> {
    veyndo_core::document::merge(doc, update)
}

/// Copies one retained member: the prior value when it had the key, otherwise the key is removed.
pub fn copy(doc: &mut Value, key: &str, prior: Option<&Value>) -> Result<()> {
    match prior {
        Some(value) => {
            object(doc)?.insert(key.into(), value.clone());
        }
        None => {
            if let Some(fields) = doc.as_object_mut() {
                fields.remove(key);
            }
        }
    }
    Ok(())
}

/// The JSON Schema node at `path`; missing nodes are created as `null` like JSON indexing does.
/// JSON Schema is an open keyword document, so the generation schema is addressed by path.
pub fn schema_at<'a>(schema: &'a mut Value, path: &[&str]) -> Result<&'a mut Value> {
    path.iter().try_fold(schema, |node, key| member(node, key))
}

/// A copy of the JSON Schema node at `path`, or `null` when the path does not exist.
pub fn schema_node(schema: &Value, path: &[&str]) -> Value {
    path.iter()
        .try_fold(schema, |node, key| node.get(key))
        .cloned()
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::{Named, copy, member, object, present, schema_at, schema_node};
    use serde::Deserialize;
    use serde_json::{Value, json};
    use veyndo_core::document::view;

    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Probe {
        #[serde(deserialize_with = "present")]
        value: Option<Value>,
    }

    #[test]
    fn views_and_writes_match_json_indexing() {
        assert_eq!(view::<Probe>(&json!({})).value, None);
        assert_eq!(
            view::<Probe>(&json!({"value":null})).value,
            Some(Value::Null)
        );
        assert_eq!(view::<Named>(&json!("text")).name, Value::Null);
        let mut doc = Value::Null;
        *member(&mut doc, "a").unwrap() = json!(1);
        let mut indexed = Value::Null;
        indexed["a"] = json!(1);
        assert_eq!(doc, indexed);
        member(&mut doc, "b").unwrap();
        indexed["b"].as_array_mut();
        assert_eq!(doc, indexed);
        copy(&mut doc, "a", None).unwrap();
        copy(&mut doc, "c", Some(&json!(true))).unwrap();
        assert_eq!(doc, json!({"b":null,"c":true}));
        assert!(object(&mut json!([1])).is_err());
        let mut schema = json!({"properties":{"a":{"type":"string"}}});
        *schema_at(&mut schema, &["properties", "b", "type"]).unwrap() = json!("null");
        assert_eq!(
            schema_node(&schema, &["properties", "b"]),
            json!({"type":"null"})
        );
        assert_eq!(schema_node(&schema, &["missing", "x"]), Value::Null);
    }
}
