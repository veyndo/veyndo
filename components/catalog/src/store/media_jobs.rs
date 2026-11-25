//! Image jobs: admission counts, the queue, private results and their publication.
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgConnection};
use veyndo_core::{Result, State};

/// The newest image job of a product.
#[derive(Serialize, FromRow)]
pub struct LatestJob {
    #[serde(skip)]
    pub product_id: String,
    pub id: String,
    pub state: String,
}

pub async fn latest(a: &State, tenant: &str, products: &[String]) -> Result<Vec<LatestJob>> {
    Ok(sqlx::query_as!(LatestJob, r#"SELECT DISTINCT ON(product_id) product_id,id,state FROM media_jobs WHERE tenant=$1 AND product_id=ANY($2) ORDER BY product_id,created_at DESC,id DESC"#, tenant, products)
        .fetch_all(&a.db)
        .await?)
}

/// Serializes image job admission of a shop, so the pending limits cannot be exceeded concurrently.
pub async fn lock_shop(tx: &mut PgConnection, tenant: &str) -> Result<()> {
    veyndo_core::store::tenants::lock(tx, tenant).await?;
    Ok(())
}

/// What an image instruction is built from, with the revision it was read at; the product is locked.
pub struct ProductFacts {
    pub revision: i64,
    pub name: String,
    pub description: String,
}

pub async fn lock_product_facts(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
) -> Result<Option<ProductFacts>> {
    Ok(sqlx::query_as!(
        ProductFacts,
        r#"SELECT revision,name,description FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant,
        product
    )
    .fetch_optional(tx)
    .await?)
}

/// Whether the asset is an uploaded raster image of this product.
pub async fn is_product_image(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    asset: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM product_assets WHERE tenant=$1 AND product_id=$2 AND id=$3 AND mime IN ('image/png','image/jpeg','image/webp')) AS "exists!""#, tenant, product, asset)
        .fetch_one(tx)
        .await?)
}

/// Queued or running jobs of the shop, optionally only those of one product.
pub async fn pending(tx: &mut PgConnection, tenant: &str, product: Option<&str>) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM media_jobs WHERE tenant=$1 AND ($2::text IS NULL OR product_id=$2) AND state IN ('queued','processing')"#, tenant, product)
        .fetch_one(tx)
        .await?)
}

pub async fn insert(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    product: &str,
    revision: i64,
    request: Value,
) -> Result<()> {
    sqlx::query!(r#"INSERT INTO media_jobs(tenant,id,product_id,product_revision,request) VALUES($1,$2,$3,$4,$5)"#, tenant, id, product, revision, request)
        .execute(tx)
        .await?;
    Ok(())
}

#[derive(Serialize, FromRow)]
pub struct JobState {
    pub id: String,
    pub state: String,
}

/// The five newest jobs of a product.
pub async fn recent(a: &State, tenant: &str, product: &str) -> Result<Vec<JobState>> {
    Ok(sqlx::query_as!(JobState, r#"SELECT id,state FROM media_jobs WHERE tenant=$1 AND product_id=$2 ORDER BY created_at DESC,id DESC LIMIT 5"#, tenant, product)
        .fetch_all(&a.db)
        .await?)
}

#[derive(FromRow)]
pub struct Outcome {
    pub state: String,
    pub error: Option<String>,
    pub asset_id: Option<String>,
}

pub async fn outcome(a: &State, tenant: &str, id: &str) -> Result<Option<Outcome>> {
    Ok(sqlx::query_as!(
        Outcome,
        r#"SELECT state,error,asset_id FROM media_jobs WHERE tenant=$1 AND id=$2"#,
        tenant,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

pub async fn asset_content(a: &State, tenant: &str, asset: &str) -> Result<Vec<u8>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT content FROM product_assets WHERE tenant=$1 AND id=$2"#,
        tenant,
        asset
    )
    .fetch_one(&a.db)
    .await?)
}

/// A job under review, locked.
#[derive(FromRow)]
pub struct Reviewed {
    pub state: String,
    pub product_id: String,
    pub product_revision: i64,
    pub asset_id: Option<String>,
}

pub async fn lock(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<Reviewed>> {
    Ok(sqlx::query_as!(Reviewed, r#"SELECT state,product_id,product_revision,asset_id FROM media_jobs WHERE tenant=$1 AND id=$2 FOR UPDATE"#, tenant, id)
        .fetch_optional(tx)
        .await?)
}

pub async fn lock_product_revision(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT revision FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant,
        product
    )
    .fetch_one(tx)
    .await?)
}

/// Publishes the reviewed image and closes its job.
pub async fn publish(tx: &mut PgConnection, tenant: &str, id: &str, asset: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE product_assets SET public=true WHERE tenant=$1 AND id=$2"#,
        tenant,
        asset
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"UPDATE media_jobs SET state='applied' WHERE tenant=$1 AND id=$2"#,
        tenant,
        id
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Interrupted provider calls are deliberately not retried: a lost response may already have incurred a charge.
pub async fn fail_interrupted(a: &State) -> Result<()> {
    sqlx::query!(r#"UPDATE media_jobs SET state='failed',error='Image request interrupted; start a new job manually' WHERE state='processing' AND started_at < now()-interval '5 minutes'"#)
        .execute(&a.db)
        .await?;
    Ok(())
}

pub struct Claimed {
    pub tenant: String,
    pub id: String,
    pub product_id: String,
    pub request: Value,
}

/// Takes the oldest queued job of an active shop; a preview environment counts with its live shop.
pub async fn claim(a: &State) -> Result<Option<Claimed>> {
    Ok(sqlx::query_as!(Claimed, r#"UPDATE media_jobs SET state='processing',started_at=now() WHERE (tenant,id)=(SELECT tenant,id FROM media_jobs WHERE state='queued' AND EXISTS(SELECT 1 FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=media_jobs.tenant),media_jobs.tenant) AND t.status='active') ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING tenant,id,product_id,request"#)
        .fetch_optional(&a.db)
        .await?)
}

pub async fn complete(a: &State, tenant: &str, id: &str, asset: &str) -> Result<()> {
    sqlx::query!(r#"UPDATE media_jobs SET state='ready',asset_id=$1 WHERE tenant=$2 AND id=$3 AND state='processing'"#, asset, tenant, id)
        .execute(&a.db)
        .await?;
    Ok(())
}

pub async fn fail(a: &State, tenant: &str, id: &str, error: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE media_jobs SET state='failed',error=$1 WHERE tenant=$2 AND id=$3"#,
        error,
        tenant,
        id
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// Type and bytes of the image an optimization starts from.
pub async fn source(
    a: &State,
    tenant: &str,
    product: &str,
    asset: &str,
) -> Result<(String, Vec<u8>)> {
    let r = sqlx::query!(
        r#"SELECT mime,content FROM product_assets WHERE tenant=$1 AND product_id=$2 AND id=$3"#,
        tenant,
        product,
        asset
    )
    .fetch_one(&a.db)
    .await?;
    Ok((r.mime, r.content))
}

/// Locks the product and counts its files.
pub async fn lock_asset_count(tx: &mut PgConnection, tenant: &str, product: &str) -> Result<i64> {
    sqlx::query!(
        r#"SELECT id FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant,
        product
    )
    .fetch_one(&mut *tx)
    .await?;
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM product_assets WHERE tenant=$1 AND product_id=$2"#,
        tenant,
        product
    )
    .fetch_one(tx)
    .await?)
}

/// Stores a generated image as a private file of the product.
pub async fn insert_generated(
    tx: &mut PgConnection,
    tenant: &str,
    asset: &str,
    product: &str,
    bytes: Vec<u8>,
    digest: &str,
) -> Result<()> {
    sqlx::query!(r#"INSERT INTO product_assets(tenant,id,product_id,title,filename,mime,kind,content,digest) VALUES($1,$2,$3,'{}','generated-image.png','image/png','attachment',$4,$5)"#, tenant, asset, product, bytes, digest)
        .execute(tx)
        .await?;
    Ok(())
}
