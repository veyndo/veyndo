//! Source-neutral category intake preserves stable hierarchy and uses the revisioned native category owner privately.
use super::SourceRecord;
use crate::ports::ImportHost;
use axum::Json;
use serde_json::{Value, json};
use veyndo_catalog::service::categories::write as write_category;
use veyndo_core::shop_scope::parent;
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
    if let Some(data) =
        sqlx::query_scalar::<_, Value>("SELECT data FROM categories WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(&id)
            .fetch_optional(&a.db)
            .await?
    {
        if data["importSource"]["digest"] == digest {
            return Ok(json!({"id":id,"replayed":true}));
        }
        return Err(conflict(
            "Source category already has a different reviewed snapshot",
        ));
    }
    let payload = mapped(a, &t, r, options).await?;
    let Json(value) = Json(write_category(&host.catalog(), h, &id, payload, true).await?);
    Ok(json!({"id":value["id"],"replayed":false}))
}

/// Read-only inspection and creation share this native hierarchy/translation mapping.
pub async fn mapped(a: &State, t: &str, r: &SourceRecord, options: &Value) -> Result<Value> {
    let namespace = super::identity_namespace(a, t).await?;
    let digest = hash(&json!({"record":r,"options":options}).to_string());
    let (settings, _) = config(a, t).await?;
    let names = r.data["title"]
        .as_object()
        .ok_or(bad("Translated category names required"))?;
    let translations:serde_json::Map<String,Value>=names.iter().map(|(locale,name)|(settings.languages().key(locale),json!({"name":name,"description":r.data["body"][locale],"slug":r.data["slug"][locale]}))).collect();
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
