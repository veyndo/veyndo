//! Source-neutral category intake preserves stable hierarchy and uses the revisioned native category owner privately.
use super::SourceRecord;
use crate::ports::ImportHost;
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_catalog::service::categories::write as write_category;
use veyndo_core::document::{lenient, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, State, bad, conflict, hash};
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::read_cache::config;
pub async fn import<H: ImportHost>(
    host: &H,
    h: &RequestContext,
    r: &SourceRecord,
    options: &Value,
) -> Result<Value> {
    let a = host.core();
    permit(h, "catalog.write")?;
    let t = merchant(h)?;
    if parent(a, &t).await?.is_none() {
        return Err(bad("Category migration requires a private preview"));
    }
    let namespace = super::identity_namespace(a, &t).await?;
    let id = r.identity(&namespace);
    let digest = hash(&json!({"record":r,"options":options}).to_string());
    if let Some(data) = crate::store::imports::category_data(a, &t, &id).await? {
        if view::<Imported>(&data).import_source.digest == digest {
            return Ok(json!({"id":id,"replayed":true}));
        }
        return Err(conflict(
            "Source category already has a different reviewed snapshot",
        ));
    }
    let payload = mapped(a, &t, r, options).await?;
    let Json(value) = Json(write_category(&host.catalog(), h, &id, payload, true).await?);
    Ok(json!({"id":view::<Created>(&value).id,"replayed":false}))
}

/// A stored category's import provenance.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Imported {
    #[serde(deserialize_with = "lenient")]
    import_source: ImportDigest,
}
/// The digest of the reviewed source snapshot.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportDigest {
    digest: Value,
}
/// The ID of a written category.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Created {
    id: Value,
}
/// The translated fields of a source category.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CategorySource {
    #[serde(deserialize_with = "lenient")]
    title: Option<serde_json::Map<String, Value>>,
    body: Value,
    slug: Value,
}
/// Read-only inspection and creation share this native hierarchy/translation mapping.
pub async fn mapped(a: &State, t: &str, r: &SourceRecord, options: &Value) -> Result<Value> {
    let namespace = super::identity_namespace(a, t).await?;
    let digest = hash(&json!({"record":r,"options":options}).to_string());
    let (settings, _) = config(a, t).await?;
    let source: CategorySource = view(&r.data);
    let names = source
        .title
        .as_ref()
        .ok_or(bad("Translated category names required"))?;
    let translations:serde_json::Map<String,Value>=names.iter().map(|(locale,name)|(settings.languages().key(locale),json!({"name":name,"description":source.body.get(locale).cloned().unwrap_or(Value::Null),"slug":source.slug.get(locale).cloned().unwrap_or(Value::Null)}))).collect();
    let parent = r
        .data
        .get("parentSourceId")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_str()
                .ok_or(bad("Category source parent must be text"))
                .and_then(|id| super::relations::identity(r, &namespace, "categories", id))
        })
        .transpose()?;
    Ok(
        json!({"revision":0,"parentId":parent,"position":0,"data":{"active":true,"visible":true,"type":"page","translations":translations,"importSource":{"digest":digest,"source":r.source,"shop":r.source_shop,"id":r.source_id}}}),
    )
}
