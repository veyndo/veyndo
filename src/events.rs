//! The outbox event schema registry: every component's typed payloads, their consumers and the contracts generated into `contracts/events/`.
//!
//! Components declare payloads with `veyndo_core::event!` in their `types::events` module. This
//! module only collects them, derives who consumes each kind from the consumers' own sources
//! (Flow Builder catalogue, the cognition lane, bundled app manifests) and renders one JSON
//! Schema per kind and version. `python3 scripts/event_contracts.py --write` writes the files
//! from `veyndo --event-contracts`; the unit test below fails when the checked-in files drift.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use veyndo_core::events::{Source, Spec, matches};

/// Every typed payload of the components, in component order.
pub(crate) fn specs() -> Vec<&'static Spec> {
    [
        veyndo_app_platform::types::events::EVENTS,
        veyndo_b2b::types::events::EVENTS,
        veyndo_catalog::types::events::EVENTS,
        veyndo_checkout::types::events::EVENTS,
        veyndo_cognition::types::events::EVENTS,
        veyndo_customers::types::events::EVENTS,
        veyndo_identity::types::events::EVENTS,
        veyndo_intelligence::types::events::EVENTS,
        veyndo_inventory::types::events::EVENTS,
        veyndo_orders::types::events::EVENTS,
        veyndo_payments::types::events::EVENTS,
        veyndo_pos::types::events::EVENTS,
        veyndo_shop::types::events::EVENTS,
        veyndo_studio::types::events::EVENTS,
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Events written by vendored code this repository does not edit: documented, not versioned here.
const EXTERNAL: &[(&str, &str, &[&str])] = &[
    (
        "app.installed",
        "extensions/services/app-storage/src/installation.rs",
        &["app", "version", "digest"],
    ),
    (
        "app.record.changed",
        "extensions/services/app-storage/src/mutations.rs",
        &["app", "entity", "id", "revision"],
    ),
    (
        "app.schema.migrated",
        "extensions/services/app-storage/src/schema_upgrade.rs",
        &["app", "fromVersion", "toVersion"],
    ),
    (
        "app.secret_rotated",
        "extensions/services/app-storage/src/secrets.rs",
        &["app", "kind", "revision", "actor"],
    ),
    (
        "app.job_controlled",
        "extensions/services/app-storage/src/job_control.rs",
        &["app", "jobId", "operation", "actor"],
    ),
    (
        "app.{app}.job_{action}",
        "extensions/services/app-storage/src/jobs.rs",
        &["jobId"],
    ),
];

/// The bundled first-party apps whose manifests subscribe to events.
const BUNDLED_APPS: &[&str] = &[
    "site_builder",
    "site_forms",
    "migration",
    "marketplaces",
    "engraving",
    "storyfront",
    "paypal",
    "shopware_payments",
    "google_analytics",
    "gmail",
    "email",
    "slack",
    "vercel",
    "klaviyo",
    "hubspot",
    "contentful",
    "algolia",
];

/// The component (`components.json` name) a producing crate belongs to.
fn component(producer: &str) -> &str {
    match producer {
        "veyndo-app-platform" => "apps",
        "veyndo-cognition" => "inference",
        other => other.trim_start_matches("veyndo-"),
    }
}

/// Whether a manifest subscription (`order.placed`, `order.*`) covers a declared kind or kind pattern.
fn subscribes(subscription: &str, kind: &str) -> bool {
    match subscription.strip_suffix(".*") {
        Some(prefix) => kind.starts_with(&format!("{prefix}.")),
        None => matches(kind, subscription),
    }
}

/// The event subscriptions of a bundled manifest.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Subscriptions {
    events: Vec<String>,
}

/// Who acts on a kind: every outbox lane that handles it, and bundled apps that subscribe to it.
fn consumers(kind: &str) -> Vec<String> {
    let mut found = vec!["audit".to_owned(), "apps".to_owned()];
    if veyndo_flow_graph::catalog::EVENTS.contains(&kind) || kind.starts_with("app.") {
        found.push("flows".into());
    }
    if veyndo_intelligence::store::observations::OBSERVED_KINDS.contains(&kind) {
        found.push("cognition".into());
    }
    for app in BUNDLED_APPS {
        let manifest: Subscriptions = veyndo_app_platform::domain::approval::built_in(app)
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or_default();
        if manifest.events.iter().any(|e| subscribes(e, kind)) {
            found.push(format!("app:{app}"));
        }
    }
    found
}

/// The file a kind's contract version lives in.
pub(crate) fn file(kind: &str, version: u32) -> String {
    format!("{kind}.v{version}.json")
}

/// The parts of a payload schema the merge reads.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Shape {
    properties: Option<Map<String, Value>>,
    required: Vec<String>,
    description: Option<String>,
}

fn source(spec: &Spec) -> &'static str {
    match spec.source {
        Source::Rust => "rust",
        Source::SqlTrigger => "sql-trigger",
    }
}

/// One kind and version: its merged contract, description and consumers.
struct Contract {
    schema: Value,
    description: String,
    consumers: Vec<String>,
}

/// The contract of one kind and version, merged over every producer that publishes it.
fn contract(kind: &str, version: u32, variants: &[&Spec]) -> Contract {
    let schemas: Vec<Value> = variants.iter().map(|s| (s.schema)()).collect();
    let shapes: Vec<Shape> = schemas.iter().map(veyndo_core::document::view).collect();
    let mut properties: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut required: Option<BTreeSet<String>> = None;
    for shape in &shapes {
        for (name, property) in shape.properties.iter().flatten() {
            let seen = properties.entry(name.clone()).or_default();
            if !seen.contains(property) {
                seen.push(property.clone());
            }
        }
        let own: BTreeSet<String> = shape.required.iter().cloned().collect();
        required = Some(match required {
            None => own,
            Some(all) => all.intersection(&own).cloned().collect(),
        });
    }
    let merged: Map<String, Value> = properties
        .into_iter()
        .map(|(name, mut seen)| {
            let property = if seen.len() == 1 {
                seen.remove(0)
            } else {
                json!({"anyOf":seen})
            };
            (name, property)
        })
        .collect();
    let producers: Vec<Value> = variants
        .iter()
        .map(|s| json!({"component":component(s.producer),"crate":s.producer,"type":s.name,"source":source(s)}))
        .collect();
    let mut description = variants
        .iter()
        .map(|s| {
            s.description
                .lines()
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" | ")
        .trim()
        .to_owned();
    let app_defined = shapes.iter().any(|s| s.properties.is_none());
    if app_defined && let Some(note) = shapes.iter().find_map(|s| s.description.as_deref()) {
        description = format!("{description} {note}");
    }
    let consumers = consumers(kind);
    let mut out = Map::new();
    out.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    out.insert("$id".into(), json!(file(kind, version)));
    out.insert("title".into(), json!(kind));
    out.insert("description".into(), json!(description));
    out.insert(
        "x-veyndo-event".into(),
        json!({"kind":kind,"version":version,"producers":producers,"consumers":consumers}),
    );
    if !app_defined {
        out.insert("type".into(), json!("object"));
        out.insert("properties".into(), Value::Object(merged));
        out.insert("required".into(), json!(required.unwrap_or_default()));
        if schemas.len() == 1 {
            out.insert("additionalProperties".into(), json!(false));
        } else {
            let titled: Vec<Value> = variants
                .iter()
                .zip(schemas)
                .map(|(s, schema)| {
                    let mut schema = schema.as_object().cloned().unwrap_or_default();
                    schema.insert(
                        "title".into(),
                        json!(format!("{} ({})", s.name, s.producer)),
                    );
                    Value::Object(schema)
                })
                .collect();
            out.insert("anyOf".into(), Value::Array(titled));
        }
    }
    Contract {
        schema: Value::Object(out),
        description,
        consumers,
    }
}

/// Every generated file under `contracts/events/` with its content.
pub(crate) fn contracts() -> BTreeMap<String, Value> {
    let mut grouped: BTreeMap<(String, u32), Vec<&Spec>> = BTreeMap::new();
    for spec in specs() {
        for kind in spec.kinds {
            grouped
                .entry(((*kind).to_owned(), spec.version))
                .or_default()
                .push(spec);
        }
    }
    let mut files = BTreeMap::new();
    let mut index = Vec::new();
    for ((kind, version), variants) in &grouped {
        let c = contract(kind, *version, variants);
        index.push(json!({
            "kind":kind,
            "version":version,
            "file":file(kind, *version),
            "description":c.description,
            "producers":variants.iter().map(|s|component(s.producer)).collect::<BTreeSet<_>>(),
            "sources":variants.iter().map(|s|source(s)).collect::<BTreeSet<_>>(),
            "consumers":c.consumers,
        }));
        files.insert(file(kind, *version), c.schema);
    }
    let external: Vec<Value> = EXTERNAL
        .iter()
        .map(|(kind, source, fields)| json!({"kind":kind,"source":source,"fields":fields,"consumers":consumers(kind)}))
        .collect();
    files.insert(
        "index.json".into(),
        json!({
            "description":"Generated by scripts/event_contracts.py from the typed payloads (veyndo_core::event!); do not edit. One file per event kind and contract version.",
            "events":index,
            "external":external,
        }),
    );
    files
}

/// The `--event-contracts` output: file name to content.
pub(crate) fn export() -> String {
    serde_json::to_string_pretty(&contracts()).expect("event contracts")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn checked_in_contracts_match_the_typed_payloads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/events");
        for (name, content) in contracts() {
            let stored = std::fs::read_to_string(root.join(&name)).unwrap_or_default();
            let stored: Value = serde_json::from_str(&stored).unwrap_or_default();
            assert_eq!(
                stored, content,
                "contracts/events/{name} is stale: cargo build --bin veyndo && python3 scripts/event_contracts.py --write"
            );
        }
    }

    #[test]
    fn every_flow_trigger_and_observed_kind_has_a_contract() {
        let all = specs();
        for kind in veyndo_flow_graph::catalog::EVENTS
            .iter()
            .chain(&veyndo_intelligence::store::observations::OBSERVED_KINDS)
        {
            assert!(
                all.iter().any(|s| veyndo_core::events::declares(s, kind)),
                "{kind} has no event contract"
            );
        }
    }

    #[test]
    fn payload_types_are_unique_and_kinds_well_formed() {
        let mut seen = BTreeSet::new();
        for spec in specs() {
            assert!(
                seen.insert((spec.producer, spec.name)),
                "{} declared twice",
                spec.name
            );
            assert!(
                !spec.description.trim().is_empty(),
                "{} lacks a description",
                spec.name
            );
            for kind in spec.kinds {
                assert!(
                    kind.split('.').count() >= 2 && !kind.contains(".."),
                    "bad kind {kind}"
                );
            }
        }
    }

    #[test]
    fn consumers_follow_their_own_sources() {
        assert!(consumers("order.placed").contains(&"cognition".to_owned()));
        assert!(consumers("order.placed").contains(&"app:email".to_owned()));
        assert!(consumers("consumer.{kind}.requested").contains(&"app:email".to_owned()));
        assert!(!consumers("team.role_changed").contains(&"flows".to_owned()));
    }
}
