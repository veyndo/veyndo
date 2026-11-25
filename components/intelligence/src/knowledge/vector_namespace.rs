//! Explicit, additive Qdrant namespace migration and alias-aware projection deletion.
use super::vectors::request;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use veyndo_core::document::{lenient, view};

fn namespace(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && value.as_bytes()[0].is_ascii_lowercase()
}
fn mapped(name: &str, from: &str) -> Option<(String, usize)> {
    let suffix = name.strip_prefix(&format!("{from}_"))?;
    let parts: Vec<_> = suffix.split('_').collect();
    if parts.len() != 4
        || !matches!(parts[0], "product" | "document")
        || parts[1] != "v2"
        || parts[2].len() != 16
        || !parts[2]
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    let dimensions: usize = parts[3].parse().ok()?;
    if !(1..=65536).contains(&dimensions) || dimensions.to_string() != parts[3] {
        return None;
    }
    Some((format!("veyndo_{suffix}"), dimensions))
}
async fn collections() -> Result<BTreeSet<String>, String> {
    let data = request(Method::GET, "/collections", None).await?;
    view::<Listing>(&data)
        .result
        .collections
        .ok_or("Invalid Qdrant collections")?
        .iter()
        .map(|c| {
            view::<Collection>(c)
                .name
                .ok_or_else(|| "Invalid Qdrant collection name".into())
        })
        .collect()
}
async fn aliases() -> Result<BTreeMap<String, String>, String> {
    let data = request(Method::GET, "/aliases", None).await?;
    view::<Listing>(&data)
        .result
        .aliases
        .ok_or("Invalid Qdrant aliases")?
        .iter()
        .map(|a| {
            let a = view::<Alias>(a);
            Ok((
                a.alias_name.ok_or("Invalid Qdrant alias name")?,
                a.collection_name.ok_or("Invalid Qdrant alias target")?,
            ))
        })
        .collect()
}
/// Run only in the explicit migration job. Keep physical collections and point IDs intact.
pub async fn migrate_configured() -> Result<usize, String> {
    let Ok(from) = std::env::var("QDRANT_NAMESPACE_MIGRATE_FROM") else {
        return Ok(0);
    };
    if from.is_empty() {
        return Ok(0);
    }
    if !namespace(&from) || from == "veyndo" {
        return Err("Invalid prior Qdrant namespace".into());
    }
    let existing = collections().await?;
    let current_aliases = aliases().await?;
    let mut actions = Vec::new();
    for old in &existing {
        let Some((new, dimensions)) = mapped(old, &from) else {
            continue;
        };
        if existing.contains(&new)
            || current_aliases
                .get(&new)
                .is_some_and(|target| target != old)
        {
            return Err("Qdrant namespace collision; no aliases changed".into());
        }
        let info = request(Method::GET, &format!("/collections/{old}"), None).await?;
        let shape = vector_shape(&info);
        if shape.size != dimensions || shape.distance != "Cosine" {
            return Err("Prior Qdrant collection shape mismatch; no aliases changed".into());
        }
        if !current_aliases.contains_key(&new) {
            actions.push(json!({"create_alias":{"collection_name":old,"alias_name":new}}));
        }
    }
    if actions.len() > 256 {
        return Err("Qdrant namespace migration exceeds collection limit".into());
    }
    let count = actions.len();
    if count > 0 {
        request(
            Method::POST,
            "/collections/aliases",
            Some(json!({"actions":actions})),
        )
        .await?;
    }
    // Reconcile ambiguous network outcomes on retry; never delete or replace an alias.
    let verified = aliases().await?;
    for old in &existing {
        if let Some((new, _)) = mapped(old, &from)
            && verified.get(&new) != Some(old)
        {
            return Err("Qdrant namespace alias verification failed".into());
        }
    }
    Ok(count)
}
pub(super) async fn names(kind: &str) -> Result<BTreeSet<String>, String> {
    if !matches!(kind, "product" | "document") {
        return Err("Invalid vector kind".into());
    }
    let mut result = collections().await?;
    result.extend(aliases().await?.into_keys());
    result.retain(|name| {
        name.starts_with(&format!("veyndo_{kind}_")) && mapped(name, "veyndo").is_some()
    });
    Ok(result)
}
/// A Qdrant listing response.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Listing {
    #[serde(deserialize_with = "lenient")]
    result: ListingResult,
}

/// Collections or aliases of a listing.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ListingResult {
    #[serde(deserialize_with = "lenient")]
    collections: Option<Vec<Value>>,
    #[serde(deserialize_with = "lenient")]
    aliases: Option<Vec<Value>>,
}

/// One listed collection.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Collection {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
}

/// One listed alias.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Alias {
    #[serde(deserialize_with = "lenient")]
    alias_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    collection_name: Option<String>,
}

/// A collection's description, down to its text vector geometry.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CollectionInfo {
    #[serde(deserialize_with = "lenient")]
    result: InfoResult,
}

/// The collection result.
#[derive(Deserialize, Default)]
#[serde(default)]
struct InfoResult {
    #[serde(deserialize_with = "lenient")]
    config: InfoConfig,
}

/// The collection configuration.
#[derive(Deserialize, Default)]
#[serde(default)]
struct InfoConfig {
    #[serde(deserialize_with = "lenient")]
    params: InfoParams,
}

/// The collection parameters.
#[derive(Deserialize, Default)]
#[serde(default)]
struct InfoParams {
    #[serde(deserialize_with = "lenient")]
    vectors: InfoVectors,
}

/// The named vectors.
#[derive(Deserialize, Default)]
#[serde(default)]
struct InfoVectors {
    #[serde(deserialize_with = "lenient")]
    text: VectorShape,
}

/// The size and distance of the text vector.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct VectorShape {
    pub size: Value,
    pub distance: Value,
}

/// The text vector geometry of a collection description.
pub(super) fn vector_shape(info: &Value) -> VectorShape {
    view::<CollectionInfo>(info)
        .result
        .config
        .params
        .vectors
        .text
}

#[cfg(test)]
mod tests {
    use super::{mapped, namespace};

    #[test]
    fn namespace_mapping_is_versioned_and_cannot_capture_foreign_or_unsafe_names() {
        let valid = "legacy_product_v2_0123456789abcdef_768";
        assert_eq!(
            mapped(valid, "legacy"),
            Some(("veyndo_product_v2_0123456789abcdef_768".into(), 768))
        );
        for value in [
            "../legacy",
            "Legacy",
            "",
            "legacy/collections",
            "legacy?",
            "legacy_",
        ] {
            assert!(!namespace(value));
        }
        for value in [
            "foreign_product_v2_0123456789abcdef_768",
            "legacy_orders_v2_0123456789abcdef_768",
            "legacy_product_v1_0123456789abcdef_768",
            "legacy_product_v2_0123456789ABCDEf_768",
            "legacy_product_v2_0123456789abcdef_0768",
            "legacy_product_v2_0123456789abcdef_0",
        ] {
            assert!(mapped(value, "legacy").is_none());
        }
    }
}
