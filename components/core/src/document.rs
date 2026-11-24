//! Typed views of stored JSON documents. A field with an unexpected shape reads as its default, exactly as indexing into the JSON yields null, so a view never rejects a document the untyped reader accepted.
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Reads the whole document as `T`; a document of another shape reads as the default.
pub fn view<T: DeserializeOwned + Default>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_default()
}

/// Field deserializer: the field as `T`, or its default when the stored value has another shape.
pub fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(view(&Value::deserialize(d)?))
}

/// Field deserializer for lists: each element as `T` or its default; anything but a list reads as empty.
pub fn lenient_list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => items.iter().map(view).collect(),
        _ => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{lenient, lenient_list, view};
    use serde::Deserialize;
    use serde_json::{Value, json};

    #[derive(Deserialize, Default, Debug, PartialEq)]
    #[serde(default)]
    struct Inner {
        a: Value,
    }
    #[derive(Deserialize, Default, Debug, PartialEq)]
    #[serde(default)]
    struct Outer {
        #[serde(deserialize_with = "lenient")]
        inner: Inner,
        #[serde(deserialize_with = "lenient_list")]
        items: Vec<Inner>,
        plain: Value,
    }

    #[test]
    fn misshaped_fields_read_like_json_indexing() {
        let o: Outer = view(&json!({"inner":null,"items":[{"a":1},"x"],"plain":2}));
        assert_eq!(o.inner, Inner::default());
        assert_eq!(o.items, vec![Inner { a: json!(1) }, Inner::default()]);
        assert_eq!(o.plain, json!(2));
        let o: Outer = view(&json!({"items":"x"}));
        assert!(o.items.is_empty());
        let o: Outer = view(&json!("not an object"));
        assert_eq!(o, Outer::default());
    }
}
