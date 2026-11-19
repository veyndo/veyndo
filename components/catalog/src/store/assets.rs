//! Reads of stored product assets.
use crate::types::asset::Upload;
use crate::types::asset::{Asset, Attachment, StoredFile};
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The first bytes (header and metadata) of a published GLB that belongs to the product or its parent.
pub async fn published_model_prefix(
    tx: &mut PgConnection,
    tenant: &str,
    asset: &str,
    product: &str,
) -> Result<Option<Vec<u8>>> {
    Ok(sqlx::query_scalar("SELECT substring(content for 1048596) AS prefix FROM product_assets WHERE tenant=$1 AND id=$2 AND public AND kind='attachment' AND mime='model/gltf-binary' AND (product_id=$3 OR product_id=(SELECT parent_id FROM products WHERE tenant=$1 AND id=$3)) FOR SHARE")
        .bind(tenant)
        .bind(asset)
        .bind(product)
        .fetch_optional(tx)
        .await?)
}

/// The newest 50 files of a product, without their content.
pub async fn of_product(a: &State, tenant: &str, product: &str) -> Result<Vec<Asset>> {
    Ok(sqlx::query_as::<_, Asset>("SELECT id,title,filename,mime,kind,public,digest,octet_length(content) AS bytes FROM product_assets WHERE tenant=$1 AND product_id=$2 ORDER BY created_at DESC LIMIT 50")
        .bind(tenant)
        .bind(product)
        .fetch_all(&a.db)
        .await?)
}

/// Locks the product so its file count cannot change concurrently; `false` when it does not exist.
pub async fn lock_product(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<bool> {
    Ok(
        sqlx::query("SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(tenant)
            .bind(product)
            .fetch_optional(tx)
            .await?
            .is_some(),
    )
}

pub async fn count(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM product_assets WHERE tenant=$1 AND product_id=$2")
            .bind(tenant)
            .bind(product)
            .fetch_one(tx)
            .await?,
    )
}

/// Stores the file unpublished.
pub async fn insert(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    product: &str,
    upload: Upload,
    digest: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO product_assets(tenant,id,product_id,title,filename,mime,kind,content,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(tenant)
        .bind(id)
        .bind(product)
        .bind(upload.title)
        .bind(upload.filename)
        .bind(upload.mime)
        .bind(upload.kind)
        .bind(upload.bytes)
        .bind(digest)
        .execute(tx)
        .await?;
    Ok(())
}

/// Publishes or withdraws exactly the content the caller saw; `false` when the digest no longer matches.
pub async fn set_public(
    a: &State,
    tenant: &str,
    id: &str,
    digest: &str,
    public: bool,
) -> Result<bool> {
    Ok(
        sqlx::query("UPDATE product_assets SET public=$1 WHERE tenant=$2 AND id=$3 AND digest=$4")
            .bind(public)
            .bind(tenant)
            .bind(id)
            .bind(digest)
            .execute(&a.db)
            .await?
            .rows_affected()
            == 1,
    )
}

/// The published attachments of a product and of its parent, oldest first.
pub async fn public_attachments(a: &State, tenant: &str, product: &str) -> Result<Vec<Attachment>> {
    Ok(sqlx::query_as::<_, Attachment>("SELECT id,title,filename,mime,octet_length(content) AS bytes FROM product_assets WHERE tenant=$1 AND public AND kind='attachment' AND (product_id=$2 OR product_id=(SELECT parent_id FROM products WHERE tenant=$1 AND id=$2)) ORDER BY created_at LIMIT 50")
        .bind(tenant)
        .bind(product)
        .fetch_all(&a.db)
        .await?)
}

pub async fn public_attachment(a: &State, tenant: &str, id: &str) -> Result<Option<StoredFile>> {
    Ok(sqlx::query_as::<_, StoredFile>("SELECT product_id,mime,filename,content FROM product_assets WHERE tenant=$1 AND id=$2 AND kind='attachment' AND public")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?)
}

/// A 3D model for the merchant preview, published or not.
pub async fn model(a: &State, tenant: &str, id: &str) -> Result<Option<StoredFile>> {
    Ok(sqlx::query_as::<_, StoredFile>("SELECT mime,filename,content FROM product_assets WHERE tenant=$1 AND id=$2 AND kind='attachment' AND mime='model/gltf-binary'")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?)
}
