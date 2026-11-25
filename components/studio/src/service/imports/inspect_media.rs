//! Current immutable image bytes and the independently editable native media association are both inspected.
use super::{SourceRecord, relations};
use crate::store::imports as db;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};

/// The reviewed fields of a source image record.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MediaSource {
    product_source_id: Value,
    digest: Value,
    byte_length: Value,
    mime: Value,
    filename: Value,
    title: Value,
    position: Value,
}
/// A media slot's ID.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Slot {
    id: Value,
}
use veyndo_core::{RequestContext, Result, State, bad, hash};
use veyndo_identity::{merchant, permit};
pub async fn inspect(
    a: &State,
    h: &RequestContext,
    r: &SourceRecord,
    namespace: &str,
    id: &str,
) -> Result<(Value, Option<Value>)> {
    permit(h, "catalog.read")?;
    let t = merchant(h)?;
    let source: MediaSource = view(&r.data);
    let product = relations::identity(
        r,
        namespace,
        "products",
        source
            .product_source_id
            .as_str()
            .ok_or(bad("Source product required"))?,
    )?;
    let mut actual = db::asset(a, &t, id).await?;
    if let Some(value) = &mut actual {
        let media = db::product_media(a, &t, &product).await?;
        merge(
            value,
            json!({ "association": media
            .and_then(|xs| {
                xs.as_array()
                    .and_then(|xs| xs.iter().find(|x| view::<Slot>(x).id == id).cloned())
            })
            .unwrap_or(Value::Null) }),
        )?;
    }
    let group = hash(&json!([r.source, r.source_shop, source.product_source_id]).to_string());
    let expected = json!({"productId":product,"digest":source.digest,"byteLength":source.byte_length,"mime":source.mime,"filename":source.filename,"title":source.title,"public":true,"association":{"id":id,"url":format!("/store-api/assets/{id}?shop={t}"),"alt":source.title,"importGroup":group,"importPosition":source.position}});
    Ok((expected, actual))
}
