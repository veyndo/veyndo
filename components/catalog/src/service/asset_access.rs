//! Asset writes and reads on behalf of imports and apps: immutable imported product images and scoped app access to the product file store.
use crate::domain::asset_file::validate_bytes;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
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
    sqlx::query("SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&t)
        .bind(product)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(bad("Import the referenced product before its images"))?;
    let previous =
        sqlx::query("SELECT product_id,digest,mime,filename,title,public FROM product_assets WHERE tenant=$1 AND id=$2")
            .bind(&t)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(previous) = previous {
        if previous.get::<String, _>("product_id") != product
            || previous.get::<String, _>("digest") != digest
            || previous.get::<String, _>("mime") != mime
            || previous.get::<String, _>("filename") != filename
            || previous.get::<Value, _>("title") != input.title
            || !previous.get::<bool, _>("public")
        {
            return Err(conflict(
                "Imported image identity already has different bytes",
            ));
        }
    } else {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM product_assets WHERE tenant=$1 AND product_id=$2",
        )
        .bind(&t)
        .bind(product)
        .fetch_one(&mut *tx)
        .await?;
        if count >= 50 {
            return Err(bad("Maximum 50 assets per product"));
        }
        sqlx::query("INSERT INTO product_assets(tenant,id,product_id,title,filename,mime,kind,public,content,digest) VALUES($1,$2,$3,$4,$5,$6,'attachment',true,$7,$8)").bind(&t).bind(id).bind(product).bind(&input.title).bind(filename).bind(mime).bind(bytes).bind(digest).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"id":id,"url":format!("/store-api/assets/{id}?shop={t}"),"view":"","alt":input.title}),
    )
}

/// App access to product files: the paged `assets` listing, or one file's bytes (`asset_preview` only for small images).
pub async fn app_files(a: &State, h: &RequestContext, operation: &str, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    permit(h, "catalog.read")?;
    if operation == "assets" {
        let product = v["productId"].as_str();
        let after = v["after"].as_str().unwrap_or("");
        if product.is_some_and(|s| s.len() > 100) || after.len() > 100 {
            return Err(bad("Invalid asset cursor"));
        }
        let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'productId',product_id,'title',title,'filename',filename,'mime',mime,'kind',kind,'public',public,'digest',digest,'bytes',octet_length(content)) FROM product_assets WHERE tenant=$1 AND ($2::text IS NULL OR product_id=$2) AND id>$3 ORDER BY id LIMIT 51").bind(&t).bind(product).bind(after).fetch_all(&a.db).await?;
        let more = rows.len() > 50;
        let items: Vec<_> = rows.into_iter().take(50).collect();
        let cursor = more.then(|| items.last().unwrap()["id"].clone());
        return Ok(json!({"elements":items,"nextCursor":cursor}));
    }
    let id = v["id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Asset ID required"))?;
    let row=sqlx::query("SELECT content,mime,digest,filename,product_id FROM product_assets WHERE tenant=$1 AND id=$2").bind(&t).bind(id).fetch_optional(&a.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Asset not found".into()))?;
    if v["productId"]
        .as_str()
        .is_some_and(|p| row.get::<String, _>("product_id") != p)
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Asset differs from surface product".into(),
        ));
    }
    let bytes: Vec<u8> = row.get("content");
    if operation == "asset_preview"
        && (!row.get::<String, _>("mime").starts_with("image/") || bytes.len() > 256 * 1024)
    {
        return Err(bad(
            "Inline asset preview requires an image of at most 256 KiB",
        ));
    }
    Ok(
        json!({"id":id,"mime":row.get::<String,_>("mime"),"filename":row.get::<String,_>("filename"),"digest":row.get::<String,_>("digest"),"base64":STANDARD.encode(bytes)}),
    )
}
