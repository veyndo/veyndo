//! The versioned outbox event contract: typed payloads declared with `event!`, their JSON Schema and the only way producers publish them.
//!
//! Every payload a component writes to the outbox is a struct declared with [`event!`] in the
//! producer's `types::events` module. The macro derives the serde shape and a JSON Schema from
//! the same field list, so the published contract (`contracts/events/<kind>.v<N>.json`) cannot
//! drift from what the producer stores. Producers publish through [`emit`] / [`emit_as`] /
//! [`emit_id_as`]; the outbox row keeps exactly the JSON the struct serializes to.
//!
//! Versions: the stored row has no version column. Version 1 payloads carry no marker; a later
//! breaking version `N >= 2` adds a required `schemaVersion: N` field, so consumers can tell the
//! versions of one kind apart while both are in the outbox.
use crate::{Error, Result, StatusCode};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// The JSON Schema of a payload field type.
pub trait Schema {
    fn schema() -> Value;
}

macro_rules! scalar {
    ($kind:literal: $($t:ty),*) => {$(impl Schema for $t { fn schema() -> Value { json!({"type": $kind}) } })*};
}
scalar!("string": str, String);
scalar!("integer": i16, i32, i64, u8, u16, u32, u64, usize);
scalar!("number": f64);
scalar!("boolean": bool);

/// Deliberately open JSON: a snapshot or merchant/app-defined data the producer passes through.
impl Schema for Value {
    fn schema() -> Value {
        json!({})
    }
}
impl<T: Schema + ?Sized> Schema for &T {
    fn schema() -> Value {
        T::schema()
    }
}
impl<T: Schema> Schema for Option<T> {
    fn schema() -> Value {
        nullable(T::schema())
    }
}
impl<T: Schema> Schema for Vec<T> {
    fn schema() -> Value {
        json!({"type":"array","items":T::schema()})
    }
}
impl<T: Schema> Schema for [T] {
    fn schema() -> Value {
        json!({"type":"array","items":T::schema()})
    }
}
impl<T: Schema> Schema for std::collections::BTreeSet<T> {
    fn schema() -> Value {
        json!({"type":"array","items":T::schema(),"uniqueItems":true})
    }
}

impl<T: Schema, S> Schema for std::collections::HashMap<String, T, S> {
    fn schema() -> Value {
        json!({"type":"object","additionalProperties":T::schema()})
    }
}
impl<T: Schema> Schema for std::collections::BTreeMap<String, T> {
    fn schema() -> Value {
        json!({"type":"object","additionalProperties":T::schema()})
    }
}

/// A schema that also admits `null`.
fn nullable(inner: Value) -> Value {
    match inner.get("type").and_then(Value::as_str) {
        Some(kind) => {
            let mut s = inner.as_object().cloned().unwrap_or_default();
            s.insert("type".into(), json!([kind, "null"]));
            Value::Object(s)
        }
        // Open JSON already admits null.
        None if inner.as_object().is_some_and(Map::is_empty) => inner,
        None => json!({"anyOf":[inner,{"type":"null"}]}),
    }
}

/// Who writes an event: compiled Rust in a component, or a trigger of a component's baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Rust,
    SqlTrigger,
}

/// The registry entry of one payload type.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// The payload type, unique within its producer.
    pub name: &'static str,
    /// The kinds this payload is published under; `{name}` stands for one segment, or for the rest as the last segment.
    pub kinds: &'static [&'static str],
    pub version: u32,
    /// The producing crate (`veyndo-<component>`).
    pub producer: &'static str,
    pub source: Source,
    pub description: &'static str,
    /// The JSON Schema of this payload type.
    pub schema: fn() -> Value,
}

/// A typed outbox payload.
pub trait Event: Serialize {
    const SPEC: Spec;
    /// The stored JSON: exactly the serde shape, plus `schemaVersion` from version 2 on.
    fn payload(&self) -> Result<Value> {
        let mut value = serde_json::to_value(self)
            .map_err(|_| Error(StatusCode::INTERNAL_SERVER_ERROR, "Event payload".into()))?;
        if Self::SPEC.version >= 2
            && let Some(object) = value.as_object_mut()
        {
            object.insert("schemaVersion".into(), json!(Self::SPEC.version));
        }
        Ok(value)
    }
}

/// Whether `kind` is one of the declared kinds or matches a declared `{segment}` pattern.
pub fn declares(spec: &Spec, kind: &str) -> bool {
    spec.kinds.iter().any(|declared| matches(declared, kind))
}

/// Whether `kind` matches a declared kind or pattern: a `{name}` segment matches one segment; as the last segment it matches the non-empty rest.
pub fn matches(pattern: &str, kind: &str) -> bool {
    let placeholder = |p: &str| p.starts_with('{') && p.ends_with('}');
    let (p, k): (Vec<_>, Vec<_>) = (pattern.split('.').collect(), kind.split('.').collect());
    let open = p.last().is_some_and(|last| placeholder(last));
    if k.iter().any(|k| k.is_empty()) || k.len() < p.len() || (!open && k.len() != p.len()) {
        return false;
    }
    p.iter().zip(&k).all(|(p, k)| placeholder(p) || p == k)
}

fn checked<E: Event>(kind: &str, event: &E) -> Result<Value> {
    if !declares(&E::SPEC, kind) {
        return Err(Error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Undeclared event kind {kind} for {}", E::SPEC.name),
        ));
    }
    event.payload()
}

/// Publishes a single-kind event in the caller's transaction (or on the pool once the change committed).
pub async fn emit<'e, E: Event>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    tenant: &str,
    event: &E,
) -> Result<()> {
    emit_as(db, tenant, E::SPEC.kinds[0], event).await
}

/// Publishes an event under one of the kinds its type declares.
pub async fn emit_as<'e, E: Event>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    tenant: &str,
    kind: &str,
    event: &E,
) -> Result<()> {
    let data = checked(kind, event)?;
    crate::store::outbox::publish_on(db, tenant, kind, &data).await
}

/// Publishes an event under one of its declared kinds and returns the outbox ID.
pub async fn emit_id_as<'e, E: Event>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    tenant: &str,
    kind: &str,
    event: &E,
) -> Result<i64> {
    let data = checked(kind, event)?;
    crate::store::outbox::publish_id_on(db, tenant, kind, &data).await
}

/// The JSON Schema of one payload type from its field list (used by `event!`).
pub fn object(version: u32, required: &[(&str, Value)], optional: &[(&str, Value)]) -> Value {
    let mut properties = Map::new();
    let mut names: Vec<&str> = Vec::new();
    for (name, schema) in required {
        properties.insert((*name).into(), schema.clone());
        names.push(name);
    }
    for (name, schema) in optional {
        properties.insert((*name).into(), schema.clone());
    }
    if version >= 2 {
        properties.insert("schemaVersion".into(), json!({"const":version}));
        names.push("schemaVersion");
    }
    names.sort_unstable();
    json!({"type":"object","properties":properties,"required":names,"additionalProperties":false})
}

/// Declares a typed outbox payload, its kinds and version; the host registry derives its consumers.
///
/// ```ignore
/// veyndo_core::event! {
///     /// An order was placed.
///     #[event(kinds = ["order.placed"], version = 1)]
///     pub struct OrderPlaced<'a> {
///         order_id: &'a str => "orderId",
///         total_minor: i64 => "totalMinor",
///     }
/// }
/// ```
/// Fields listed in a trailing `optional { .. }` block are left out while `None` and are not
/// required by the schema: the only way to extend a published version without breaking it.
#[macro_export]
macro_rules! event {
    (
        $(#[doc = $doc:literal])*
        #[event(kinds = [$($kind:literal),+ $(,)?], version = $version:literal $(, source = $source:ident)?)]
        pub struct $name:ident $(<$lt:lifetime>)? {
            $($field:ident : $ty:ty => $wire:literal),* $(,)?
        }
        $(optional { $($ofield:ident : $oty:ty => $owire:literal),* $(,)? })?
    ) => {
        $(#[doc = $doc])*
        #[derive(::serde::Serialize, Debug, Clone)]
        pub struct $name $(<$lt>)? {
            $(#[serde(rename = $wire)] pub $field: $ty,)*
            $($(#[serde(rename = $owire, skip_serializing_if = "Option::is_none")] pub $ofield: Option<$oty>,)*)?
        }
        impl $(<$lt>)? $crate::events::Event for $name $(<$lt>)? {
            const SPEC: $crate::events::Spec = $crate::events::Spec {
                name: stringify!($name),
                kinds: &[$($kind),+],
                version: $version,
                producer: env!("CARGO_PKG_NAME"),
                source: $crate::event!(@source $($source)?),
                description: concat!($($doc, "\n"),*),
                schema: || $crate::events::object(
                    $version,
                    &[$(($wire, <$ty as $crate::events::Schema>::schema())),*],
                    &[$($(($owire, <$oty as $crate::events::Schema>::schema())),*)?],
                ),
            };
        }
    };
    (
        $(#[doc = $doc:literal])*
        #[event(kinds = [$($kind:literal),+ $(,)?], version = $version:literal)]
        pub struct $name:ident(app_defined Value);
    ) => {
        $(#[doc = $doc])*
        #[derive(::serde::Serialize, Debug, Clone)]
        #[serde(transparent)]
        pub struct $name(pub ::serde_json::Value);
        impl $crate::events::Event for $name {
            const SPEC: $crate::events::Spec = $crate::events::Spec {
                name: stringify!($name),
                kinds: &[$($kind),+],
                version: $version,
                producer: env!("CARGO_PKG_NAME"),
                source: $crate::events::Source::Rust,
                description: concat!($($doc, "\n"),*),
                schema: || ::serde_json::json!({"description":"App-defined payload: the app's manifest declares its shape."}),
            };
        }
    };
    (@source) => { $crate::events::Source::Rust };
    (@source SqlTrigger) => { $crate::events::Source::SqlTrigger };
}

/// Declares a typed object nested in an event payload (same field syntax as `event!`, no kinds).
#[macro_export]
macro_rules! event_part {
    (
        $(#[doc = $doc:literal])*
        pub struct $name:ident $(<$lt:lifetime>)? {
            $($field:ident : $ty:ty => $wire:literal),* $(,)?
        }
    ) => {
        $(#[doc = $doc])*
        #[derive(::serde::Serialize, Debug, Clone)]
        pub struct $name $(<$lt>)? {
            $(#[serde(rename = $wire)] pub $field: $ty,)*
        }
        impl $(<$lt>)? $crate::events::Schema for $name $(<$lt>)? {
            fn schema() -> ::serde_json::Value {
                $crate::events::object(1, &[$(($wire, <$ty as $crate::events::Schema>::schema())),*], &[])
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    crate::event! {
        /// A probe.
        #[event(kinds = ["probe.changed", "probe.{kind}.done"], version = 1)]
        pub struct Probe<'a> {
            id: &'a str => "probeId",
            count: i64 => "count",
            note: Option<&'a str> => "note",
            tags: Vec<String> => "tags",
            data: Value => "data",
        }
        optional { later: bool => "later" }
    }
    crate::event! {
        /// A second version.
        #[event(kinds = ["probe.changed"], version = 2)]
        pub struct ProbeV2 { id: String => "id" }
    }

    #[test]
    fn payload_is_the_serde_shape_and_schema_follows_the_fields() {
        let p = Probe {
            id: "p",
            count: 2,
            note: None,
            tags: vec![],
            data: json!({"x":1}),
            later: None,
        };
        assert_eq!(
            p.payload().unwrap(),
            json!({"probeId":"p","count":2,"note":null,"tags":[],"data":{"x":1}})
        );
        let schema = (Probe::SPEC.schema)();
        assert_eq!(
            schema["required"],
            json!(["count", "data", "note", "probeId", "tags"])
        );
        assert_eq!(
            schema["properties"]["note"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(schema["properties"]["later"]["type"], "boolean");
        assert_eq!(Probe::SPEC.producer, "veyndo-core");
        assert_eq!(Probe::SPEC.description, " A probe.\n");
        let v2 = ProbeV2 { id: "i".into() }.payload().unwrap();
        assert_eq!(v2, json!({"id":"i","schemaVersion":2}));
        assert_eq!(
            (ProbeV2::SPEC.schema)()["required"],
            json!(["id", "schemaVersion"])
        );
    }

    #[test]
    fn only_declared_kinds_are_accepted() {
        let p = ProbeV2 { id: "i".into() };
        assert!(checked("probe.changed", &p).is_ok());
        assert!(checked("probe.other", &p).is_err());
        assert!(declares(&Probe::SPEC, "probe.consent.done"));
        assert!(!declares(&Probe::SPEC, "probe.consent.extra.done"));
        assert!(!declares(&Probe::SPEC, "probe..done"));
        let open = Spec {
            kinds: &["app.{app}.{event}"],
            ..Probe::SPEC
        };
        assert!(declares(&open, "app.slack.job_sync"));
        assert!(declares(&open, "app.slack.order.sent"));
        assert!(!declares(&open, "app.slack"));
        assert!(!declares(&open, "app.slack."));
        assert!(!declares(&open, "order.slack.sent"));
    }
}
