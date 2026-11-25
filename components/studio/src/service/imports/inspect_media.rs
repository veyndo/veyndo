//! Current immutable image bytes and the independently editable native media association are both inspected.
use super::{SourceRecord, relations};
use crate::store::imports as db;
use serde_json::{Value, json};
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
    let product = relations::identity(
        r,
        namespace,
        "products",
        r.data["productSourceId"]
            .as_str()
            .ok_or(bad("Source product required"))?,
    )?;
    let mut actual = db::asset(a, &t, id).await?;
    if let Some(value) = &mut actual {
        let media = db::product_media(a, &t, &product).await?;
        value["association"] = media
            .and_then(|xs| {
                xs.as_array()
                    .and_then(|xs| xs.iter().find(|x| x["id"] == id).cloned())
            })
            .unwrap_or(Value::Null);
    }
    let group = hash(&json!([r.source, r.source_shop, r.data["productSourceId"]]).to_string());
    let expected = json!({"productId":product,"digest":r.data["digest"],"byteLength":r.data["byteLength"],"mime":r.data["mime"],"filename":r.data["filename"],"title":r.data["title"],"public":true,"association":{"id":id,"url":format!("/store-api/assets/{id}?shop={t}"),"alt":r.data["title"],"importGroup":group,"importPosition":r.data["position"]}});
    Ok((expected, actual))
}
