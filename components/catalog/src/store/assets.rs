//! Reads of stored product assets.
use crate::types::asset::Upload;
use crate::types::asset::{Asset, Attachment, StoredFile};
use serde_json::Value;
use sqlx::{PgConnection, Row};
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

/// The stored identity of an imported image: its product, bytes digest, format, file name, title and publication.
pub struct ImportedIdentity {
    pub product_id: String,
    pub digest: String,
    pub mime: String,
    pub filename: String,
    pub title: Value,
    pub public: bool,
}

pub async fn imported_identity(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<ImportedIdentity>> {
    Ok(sqlx::query("SELECT product_id,digest,mime,filename,title,public FROM product_assets WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(tx)
        .await?
        .map(|row| ImportedIdentity {
            product_id: row.get("product_id"),
            digest: row.get("digest"),
            mime: row.get("mime"),
            filename: row.get("filename"),
            title: row.get("title"),
            public: row.get("public"),
        }))
}

/// An imported product image, stored published as an attachment.
pub struct NewImport<'a> {
    pub id: &'a str,
    pub product: &'a str,
    pub title: &'a Value,
    pub filename: &'a str,
    pub mime: &'a str,
    pub bytes: Vec<u8>,
    pub digest: String,
}

pub async fn insert_imported(
    tx: &mut PgConnection,
    tenant: &str,
    image: NewImport<'_>,
) -> Result<()> {
    sqlx::query("INSERT INTO product_assets(tenant,id,product_id,title,filename,mime,kind,public,content,digest) VALUES($1,$2,$3,$4,$5,$6,'attachment',true,$7,$8)")
        .bind(tenant)
        .bind(image.id)
        .bind(image.product)
        .bind(image.title)
        .bind(image.filename)
        .bind(image.mime)
        .bind(image.bytes)
        .bind(image.digest)
        .execute(tx)
        .await?;
    Ok(())
}

/// Up to 51 file descriptions after the ID cursor, optionally of one product, for apps.
pub async fn app_page(
    a: &State,
    tenant: &str,
    product: Option<&str>,
    after: &str,
) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('id',id,'productId',product_id,'title',title,'filename',filename,'mime',mime,'kind',kind,'public',public,'digest',digest,'bytes',octet_length(content)) FROM product_assets WHERE tenant=$1 AND ($2::text IS NULL OR product_id=$2) AND id>$3 ORDER BY id LIMIT 51")
        .bind(tenant)
        .bind(product)
        .bind(after)
        .fetch_all(&a.db)
        .await?)
}

/// One file with its content, for apps.
pub struct AppFile {
    pub content: Vec<u8>,
    pub mime: String,
    pub digest: String,
    pub filename: String,
    pub product_id: String,
}

pub async fn app_file(a: &State, tenant: &str, id: &str) -> Result<Option<AppFile>> {
    Ok(sqlx::query("SELECT content,mime,digest,filename,product_id FROM product_assets WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .map(|row| AppFile {
            content: row.get("content"),
            mime: row.get("mime"),
            digest: row.get("digest"),
            filename: row.get("filename"),
            product_id: row.get("product_id"),
        }))
}
