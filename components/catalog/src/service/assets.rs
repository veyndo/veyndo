//! File admission, immutable bytes and explicit publishing; binary content never enters merchant list responses.
use crate::domain::asset_file::validate_bytes;
use crate::ports::Catalog;
use crate::store::assets;
use crate::types::asset::Upload;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, uid};
use veyndo_identity::{merchant, permit};

/// Stores an uploaded file for a product, unpublished; a product holds at most 50 files.
pub async fn persist(
    c: &Catalog,
    h: &RequestContext,
    product: &str,
    upload: Upload,
) -> Result<Value> {
    let tenant = merchant(h)?;
    permit(h, "catalog.write")?;
    c.shop
        .languages(&tenant)
        .await?
        .as_ref()
        .validate_names(&upload.title, 200)?;
    validate_bytes(&upload.mime, &upload.bytes)?;
    let id = uid();
    let digest = format!("{:x}", Sha256::digest(&upload.bytes));
    let mut tx = c.core.db.begin().await?;
    if !assets::lock_product(&mut tx, &tenant, product).await? {
        return Err(Error(StatusCode::NOT_FOUND, "Product not found".into()));
    }
    if assets::count(&mut tx, &tenant, product).await? >= 50 {
        return Err(bad("Maximum 50 assets per product"));
    }
    assets::insert(&mut tx, &tenant, &id, product, upload, &digest).await?;
    tx.commit().await?;
    Ok(json!({"id":id,"shop":tenant,"digest":digest,"public":false}))
}
