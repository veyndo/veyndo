//! Typed API and bounded upload write source hashes, chunks and graph relations atomically.
use crate::domain::source_content::admit_guidance;
use crate::service::pdf::text;
use crate::service::sources::validate_content;
use crate::store::documents as store;
use axum::Json;
use axum::extract::{Multipart, Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{merge, view};
use veyndo_core::shop_scope::tenant;
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::{merchant, permit};

/// `POST /api/knowledge/documents`: a source sent as JSON.
pub async fn ingest(
    Shared(a): Shared<State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    merchant(&h)?;
    permit(&h, "knowledge.write")?;
    Ok(Json(save(&a, &h, &tenant(&h)?, &v, "api").await?))
}
/// `POST /api/knowledge/documents/upload`: a bounded text or PDF upload with its metadata.
pub async fn upload(
    Shared(a): Shared<State>,
    h: RequestContext,
    mut form: Multipart,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.write")?;
    let mut v = json!({});
    let mut file = None;
    let mut names = std::collections::HashSet::new();
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|_| bad("Invalid multipart document"))?
    {
        let name = field.name().unwrap_or("").to_owned();
        if !names.insert(name.clone()) {
            return Err(bad("Duplicate upload field"));
        }
        if name == "file" {
            let pdf = field
                .file_name()
                .is_some_and(|n| n.to_lowercase().ends_with(".pdf"));
            let bytes = field
                .bytes()
                .await
                .map_err(|_| bad("Invalid document upload"))?;
            file = Some((bytes.to_vec(), pdf));
        } else if name == "translations" {
            let text = field
                .text()
                .await
                .map_err(|_| bad("Invalid translations"))?;
            if text.len() > 200_000 {
                return Err(bad("Translations too long"));
            }
            v[name] = serde_json::from_str(&text).map_err(|_| bad("Invalid translation JSON"))?;
        } else if ["title", "productId", "kind", "locale"].contains(&name.as_str()) {
            let text = field
                .text()
                .await
                .map_err(|_| bad("Invalid document metadata"))?;
            if text.len() > 200 {
                return Err(bad("Metadata too long"));
            }
            v[name] = json!(text);
        } else {
            return Err(bad("Unsupported upload field"));
        }
    }
    let (bytes, pdf) = file.ok_or(bad("File required"))?;
    merge(&mut v, json!({ "content": text(bytes, pdf).await? }))?;
    Ok(Json(
        save(&a, &h, &t, &v, if pdf { "pdf" } else { "upload-text" }).await?,
    ))
}
/// The JSON fields `save` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SaveInput {
    kind: Value,
}

/// Admits guidance, validates the content against the shop languages and stores it with its chunks.
async fn save(a: &State, h: &RequestContext, t: &str, v: &Value, source: &str) -> Result<Value> {
    let input: SaveInput = view(v);
    admit_guidance(h, &input.kind)?;
    let data = validate_content(a, t, v).await?;
    store::save(a, h, t, &data, source).await
}
/// `GET /api/knowledge/documents`.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.read")?;
    Ok(Json(store::list(&a, &t).await?))
}
/// `PUT /api/knowledge/documents/{id}`: the explicit publication decision of a source.
pub async fn publish(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.write")?;
    Ok(Json(store::publish(&a, &h, &t, &id, &v).await?))
}
