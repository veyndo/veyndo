//! Asset writes and reads on behalf of imports and apps: immutable imported product images and scoped app access to the product file store.
use crate::domain::asset_file::validate_bytes;
use crate::store::assets::{self, NewImport};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_core::document::{lenient, view};
use veyndo_core::localized::Languages;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{merchant, permit};

/// Stores one imported product image once: the same identity with different bytes or metadata is a conflict, and a product holds at most 50 assets.
/// The metadata of an imported product image: its format, file name, localized title and content digest.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportedImage {
    #[serde(deserialize_with = "lenient")]
    mime: Option<String>,
    #[serde(deserialize_with = "lenient")]
    filename: Option<String>,
    title: Value,
    #[serde(deserialize_with = "lenient")]
    digest: Option<String>,
}

pub async fn import_image(
    a: &State,
    h: &RequestContext,
    (product, id): (&str, &str),
    metadata: &Value,
    bytes: Vec<u8>,
    languages: Languages<'_>,
) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "catalog.write")?;
    let input: ImportedImage = view(metadata);
    let mime = input
        .mime
        .as_deref()
        .filter(|m| ["image/png", "image/jpeg", "image/webp"].contains(m))
        .ok_or(bad("Imported image format unavailable"))?;
    validate_bytes(mime, &bytes)?;
    let filename = input
        .filename
        .as_deref()
        .filter(|n| !n.is_empty() && n.len() <= 180 && !n.contains(['/', '\\', '\r', '\n', '"']))
        .ok_or(bad("Invalid imported image filename"))?;
    languages.validate_names(&input.title, 200)?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if input.digest.as_deref() != Some(digest.as_str()) {
        return Err(bad("Imported image digest differs"));
    }
    let mut tx = a.db.begin().await?;
    if !assets::lock_product(&mut tx, &t, product).await? {
        return Err(bad("Import the referenced product before its images"));
    }
    if let Some(previous) = assets::imported_identity(&mut tx, &t, id).await? {
        if previous.product_id != product
            || previous.digest != digest
            || previous.mime != mime
            || previous.filename != filename
            || previous.title != input.title
            || !previous.public
        {
            return Err(conflict(
                "Imported image identity already has different bytes",
            ));
        }
    } else {
        if assets::count(&mut tx, &t, product).await? >= 50 {
            return Err(bad("Maximum 50 assets per product"));
        }
        let image = NewImport {
            id,
            product,
            title: &input.title,
            filename,
            mime,
            bytes,
            digest,
        };
        assets::insert_imported(&mut tx, &t, image).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"id":id,"url":format!("/store-api/assets/{id}?shop={t}"),"view":"","alt":input.title}),
    )
}

/// An app file request: a product, a page cursor or one file.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FileRequest {
    product_id: Value,
    #[serde(deserialize_with = "lenient")]
    after: Option<String>,
    id: Value,
}

/// App access to product files: the paged `assets` listing, or one file's bytes (`asset_preview` only for small images).
pub async fn app_files(a: &State, h: &RequestContext, operation: &str, v: &Value) -> Result<Value> {
    let input: FileRequest = view(v);
    let t = merchant(h)?;
    permit(h, "catalog.read")?;
    if operation == "assets" {
        let product = input.product_id.as_str();
        let after = input.after.as_deref().unwrap_or("");
        if product.is_some_and(|s| s.len() > 100) || after.len() > 100 {
            return Err(bad("Invalid asset cursor"));
        }
        let rows = assets::app_page(a, &t, product, after).await?;
        let more = rows.len() > 50;
        let items: Vec<_> = rows.into_iter().take(50).collect();
        let cursor = more.then(|| items.last().unwrap()["id"].clone());
        return Ok(json!({"elements":items,"nextCursor":cursor}));
    }
    let id = input
        .id
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Asset ID required"))?;
    let file = assets::app_file(a, &t, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Asset not found".into()))?;
    if input
        .product_id
        .as_str()
        .is_some_and(|p| file.product_id != p)
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Asset differs from surface product".into(),
        ));
    }
    let bytes = file.content;
    if operation == "asset_preview"
        && (!file.mime.starts_with("image/") || bytes.len() > 256 * 1024)
    {
        return Err(bad(
            "Inline asset preview requires an image of at most 256 KiB",
        ));
    }
    Ok(
        json!({"id":id,"mime":file.mime,"filename":file.filename,"digest":file.digest,"base64":STANDARD.encode(bytes)}),
    )
}
