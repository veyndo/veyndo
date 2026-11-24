//! Typed views of stored JSON documents. A field with an unexpected shape reads as its default, exactly as indexing into the JSON yields null, so a view never rejects a document the untyped reader accepted.
use serde::de::{DeserializeOwned, Error as _, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Writes a typed update into the object at `slot`: each serialized field replaces the stored one,
/// other stored fields stay. A missing (null) slot becomes an object, exactly as assigning through
/// JSON indexing did; any other shape is an error instead of the panic indexing would raise.
pub fn merge(slot: &mut Value, update: impl Serialize) -> crate::Result<()> {
    let shape = || {
        crate::Error(
            crate::StatusCode::INTERNAL_SERVER_ERROR,
            "Stored document has an unexpected shape".into(),
        )
    };
    let Ok(Value::Object(fields)) = serde_json::to_value(update) else {
        return Err(shape());
    };
    if slot.is_null() {
        *slot = Value::Object(serde_json::Map::new());
    }
    slot.as_object_mut().ok_or_else(shape)?.extend(fields);
    Ok(())
}

/// Reads the whole document as `T`; a document of another shape reads as the default.
/// A record (struct) reads only from an object: serde would also accept a positional array, where indexing yields null.
pub fn view<T: DeserializeOwned + Default>(v: &Value) -> T {
    T::deserialize(Record(v.clone())).unwrap_or_default()
}

/// A JSON value that refuses to deserialize a struct from anything but an object.
struct Record(Value);

macro_rules! forward {
    ($($method:ident),*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.0.$method(visitor)
        }
    )*};
}

impl<'de> Deserializer<'de> for Record {
    type Error = serde_json::Error;
    forward!(
        deserialize_any,
        deserialize_bool,
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_f32,
        deserialize_f64,
        deserialize_char,
        deserialize_str,
        deserialize_string,
        deserialize_bytes,
        deserialize_byte_buf,
        deserialize_option,
        deserialize_unit,
        deserialize_seq,
        deserialize_map,
        deserialize_identifier,
        deserialize_ignored_any
    );
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_unit_struct(name, visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_newtype_struct(name, visitor)
    }
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_tuple(len, visitor)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_tuple_struct(name, len, visitor)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_enum(name, variants, visitor)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        if self.0.is_object() {
            self.0.deserialize_struct(name, fields, visitor)
        } else {
            Err(Self::Error::custom("a record reads only from an object"))
        }
    }
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
    use super::{lenient, lenient_list, merge, view};
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

    #[derive(serde::Serialize)]
    struct Update {
        a: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<i32>,
    }

    #[test]
    fn a_merge_writes_like_indexed_assignment_and_refuses_other_shapes() {
        let mut v = json!({"a":0,"b":1,"c":2});
        merge(&mut v, Update { a: 5, b: None }).unwrap();
        assert_eq!(v, json!({"a":5,"b":1,"c":2}));
        let mut missing = Value::Null;
        merge(&mut missing, Update { a: 1, b: Some(2) }).unwrap();
        assert_eq!(missing, json!({"a":1,"b":2}));
        assert!(merge(&mut json!("text"), Update { a: 1, b: None }).is_err());
        assert!(merge(&mut json!({}), 7).is_err());
    }

    #[derive(Deserialize, Default, Debug)]
    #[serde(default)]
    struct Fields {
        #[serde(deserialize_with = "lenient")]
        s: Option<String>,
        #[serde(deserialize_with = "lenient")]
        i: Option<i64>,
        #[serde(deserialize_with = "lenient")]
        u: Option<u64>,
        #[serde(deserialize_with = "lenient")]
        f: Option<f64>,
        #[serde(deserialize_with = "lenient")]
        b: Option<bool>,
        #[serde(deserialize_with = "lenient")]
        a: Option<Vec<Value>>,
        #[serde(deserialize_with = "lenient")]
        o: Option<serde_json::Map<String, Value>>,
        raw: Value,
    }

    /// The equivalence request inputs rely on: every lenient field reads exactly what the matching
    /// accessor on the indexed JSON value returned, for every shape a client can send.
    #[test]
    fn lenient_fields_equal_the_json_accessors() {
        let shapes = [
            json!(null),
            json!(true),
            json!(false),
            json!(0),
            json!(-1),
            json!(7),
            json!(u64::MAX),
            json!(i64::MIN),
            json!(2.0),
            json!(2.5),
            json!(-0.0),
            json!(""),
            json!("x"),
            json!("7"),
            json!([]),
            json!([1, "a"]),
            json!({}),
            json!({"k":1}),
        ];
        for shape in &shapes {
            let doc = json!({"s":shape,"i":shape,"u":shape,"f":shape,"b":shape,"a":shape,"o":shape,"raw":shape});
            let typed: Fields = view(&doc);
            assert_eq!(typed.s.as_deref(), doc["s"].as_str(), "{shape}");
            assert_eq!(typed.i, doc["i"].as_i64(), "{shape}");
            assert_eq!(typed.u, doc["u"].as_u64(), "{shape}");
            assert_eq!(typed.f, doc["f"].as_f64(), "{shape}");
            assert_eq!(typed.b, doc["b"].as_bool(), "{shape}");
            assert_eq!(typed.a.as_ref(), doc["a"].as_array(), "{shape}");
            assert_eq!(typed.o.as_ref(), doc["o"].as_object(), "{shape}");
            assert_eq!(&typed.raw, &doc["raw"], "{shape}");
            assert_eq!(typed.b == Some(true), doc["b"] == true, "{shape}");
            assert_eq!(typed.s.as_deref() == Some("x"), doc["s"] == "x", "{shape}");
        }
        let missing: Fields = view(&json!({}));
        assert!(missing.s.is_none() && missing.i.is_none() && missing.raw.is_null());
        let not_object: Fields = view(&json!([1, 2, 3, 4, 5, 6, 7, 8]));
        assert!(not_object.s.is_none() && not_object.raw.is_null());
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
        let o: Outer = view(&json!({"inner":[7],"items":[[1]],"plain":[3]}));
        assert_eq!(o.inner, Inner::default());
        assert_eq!(o.items, vec![Inner::default()]);
        assert_eq!(o.plain, json!([3]));
        assert_eq!(view::<Outer>(&json!([{"a":1}, [], 2])), Outer::default());
        assert_eq!(view::<Vec<String>>(&json!(["a", "b"])), vec!["a", "b"]);
        assert_eq!(view::<Option<String>>(&json!(null)), None);
    }
}
