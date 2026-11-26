//! Reviewed source image intake resolves its native product and delegates immutable binary storage and media edits to their existing owners.
use super::SourceRecord;
use crate::http::product_editor::{edit_product, product_editor};
use crate::ports::ImportHost;
use axum::Json;
use axum::extract::{Path, State as Shared};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::store::environments::parent;
use veyndo_core::{RequestContext, Result, bad, conflict, hash};
use veyndo_identity::{merchant, permit};
/// The reviewed fields of a source image record.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImageSource {
    #[serde(deserialize_with = "lenient")]
    byte_length: Option<u64>,
    digest: Value,
    product_source_id: Value,
    #[serde(deserialize_with = "lenient")]
    position: Option<u64>,
}
/// The product editor document an image edit resubmits.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EditorDocument {
    revision: Value,
    content_revision: Value,
    translations: Value,
    extra: Value,
    catalog: Value,
    #[serde(deserialize_with = "lenient")]
    commerce: Map<String, Value>,
}
/// A media slot's identity and import placement.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MediaSlot {
    id: Value,
    import_group: Value,
    #[serde(deserialize_with = "lenient")]
    import_position: Option<u64>,
}
/// Imports one reviewed source image into its product's media.
pub async fn import<H: ImportHost>(
    host: &H,
    h: &RequestContext,
    r: &SourceRecord,
    chunks: &Value,
) -> Result<Value> {
    let a = host.core();
    permit(h, "catalog.write")?;
    permit(h, "catalog.read")?;
    let t = merchant(h)?;
    if parent(a, &t).await?.is_none() {
        return Err(bad("Image migration requires a private preview"));
    }
    let chunks = chunks
        .as_array()
        .filter(|xs| !xs.is_empty() && xs.len() <= 172)
        .ok_or(bad("Bounded asset chunks required"))?;
    let mut encoded = String::new();
    for chunk in chunks {
        let s = chunk
            .as_str()
            .filter(|s| s.len() <= 65536)
            .ok_or(bad("Invalid asset chunk"))?;
        if encoded.len() + s.len() > 11_184_812 {
            return Err(bad("Imported image exceeds 8 MiB"));
        }
        encoded.push_str(s);
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| bad("Invalid asset encoding"))?;
    let source: ImageSource = view(&r.data);
    if bytes.is_empty()
        || bytes.len() > 8 * 1024 * 1024
        || source.byte_length != Some(bytes.len() as u64)
        || source.digest != hash_bytes(&bytes)
    {
        return Err(bad("Image bytes differ from reviewed source digest"));
    }
    let namespace = super::identity_namespace(a, &t).await?;
    let product = super::relations::identity(
        r,
        &namespace,
        "products",
        source
            .product_source_id
            .as_str()
            .ok_or(bad("Source product required"))?,
    )?;
    let id = r.identity(&namespace);
    let position = source
        .position
        .filter(|p| *p < 20)
        .ok_or(bad("Reviewed source image position must be 0..19"))?;
    let group = hash(&json!([r.source, r.source_shop, source.product_source_id]).to_string());
    let mut media = host.import_image(h, &product, &id, &r.data, bytes).await?;
    merge(&mut media, json!({ "importGroup": group }))?;
    merge(&mut media, json!({ "importPosition": position }))?;
    let Json(editor) = product_editor(Shared(a.clone()), h.clone(), Path(product.clone())).await?;
    let mut editor: EditorDocument = view(&editor);
    let items = editor
        .commerce
        .get_mut("media")
        .and_then(Value::as_array_mut)
        .ok_or(bad("Native media unavailable"))?;
    if let Some(existing) = items.iter().find(|x| view::<MediaSlot>(x).id == id) {
        if existing != &media {
            return Err(conflict(
                "Imported media association was edited; review it before continuing",
            ));
        }
        return Ok(json!({"id":id,"replayed":true}));
    }
    items.push(media);
    // Reorder only this source group's slots. Manual media retain their positions and content.
    let mut imported = items
        .iter()
        .filter(|m| view::<MediaSlot>(m).import_group == group)
        .cloned()
        .collect::<Vec<_>>();
    imported.sort_by_key(|m| view::<MediaSlot>(m).import_position.unwrap_or(u64::MAX));
    let mut ordered = imported.into_iter();
    for item in items
        .iter_mut()
        .filter(|m| view::<MediaSlot>(m).import_group == group)
    {
        *item = ordered.next().unwrap();
    }
    // Omit stock so adding an image never restores a quantity observed before a concurrent native writer.
    editor.commerce.remove("stock");
    let payload = json!({"revision":editor.revision,"contentRevision":editor.content_revision,"translations":editor.translations,"extra":editor.extra,"catalog":editor.catalog,"commerce":editor.commerce});
    let _ = edit_product(Shared(a.clone()), h.clone(), Path(product), Json(payload)).await?;
    Ok(json!({"id":id,"replayed":false}))
}
fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
