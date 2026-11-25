//! Typed writes into nested fields of stored JSON documents: `field` reaches a child slot exactly as JSON index assignment does, so a typed patch (`document::merge`) can land below the top level.
use serde_json::{Map, Value};

/// The field `key` of the object at `slot`, created as null when missing, exactly as `slot["key"]` assignment reaches it.
/// A null slot becomes an empty object first; any other shape is an error instead of the panic indexing would raise.
pub fn field<'a>(slot: &'a mut Value, key: &str) -> crate::Result<&'a mut Value> {
    if slot.is_null() {
        *slot = Value::Object(Map::new());
    }
    slot.as_object_mut()
        .map(|fields| fields.entry(key).or_insert(Value::Null))
        .ok_or_else(|| {
            crate::Error(
                crate::StatusCode::INTERNAL_SERVER_ERROR,
                "Stored document has an unexpected shape".into(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::field;
    use serde_json::{Value, json};

    /// Every shape indexing accepts: the typed slot writes the same document; every shape it panics on is an error.
    #[test]
    fn slots_write_like_index_assignment() {
        for doc in [
            json!(null),
            json!({}),
            json!({"a":null}),
            json!({"a":{"b":2}}),
        ] {
            let mut indexed = doc.clone();
            indexed["a"]["c"] = json!(3);
            let mut typed = doc.clone();
            *field(field(&mut typed, "a").unwrap(), "c").unwrap() = json!(3);
            assert_eq!(typed, indexed, "{doc}");
            let mut touched = doc.clone();
            let _ = indexed["x"].as_object_mut();
            let _ = field(&mut touched, "x").unwrap().as_object_mut();
            touched["a"]["c"] = json!(3);
            assert_eq!(touched, indexed, "{doc}");
        }
        assert!(field(&mut json!("text"), "a").is_err());
        assert!(field(field(&mut json!({"a":1}), "a").unwrap(), "c").is_err());
        assert!(field(&mut json!([1]), "a").is_err());
        let mut missing = Value::Null;
        field(&mut missing, "k").unwrap();
        assert_eq!(missing, json!({"k":null}));
    }
}
