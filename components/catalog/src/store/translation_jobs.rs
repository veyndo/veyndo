//! Translation jobs and their per-product drafts, as far as applying reviewed drafts needs them.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// How many of the given products exist in the shop.
pub async fn count_products(tx: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM products WHERE tenant=$1 AND id=ANY($2)"#,
        tenant,
        ids
    )
    .fetch_one(tx)
    .await?)
}

/// The languages of a job that is ready to be applied, locked.
pub struct ReadyJob {
    pub source_locale: String,
    pub target_locale: String,
}

pub async fn lock_ready(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<ReadyJob>> {
    Ok(sqlx::query_as!(ReadyJob, r#"SELECT source_locale,target_locale FROM translation_jobs WHERE tenant=$1 AND id=$2 AND status='ready' FOR UPDATE"#, tenant, id)
    .fetch_optional(tx)
    .await?)
}

/// A reviewed draft for one product, made against `revision` of that product.
pub struct Draft {
    pub product_id: String,
    pub revision: i64,
    pub result: Value,
}

/// Up to 50 ready drafts of a job in product order, locked; optionally only one product.
pub async fn lock_drafts(
    tx: &mut PgConnection,
    tenant: &str,
    job: &str,
    product: Option<&str>,
) -> Result<Vec<Draft>> {
    Ok(sqlx::query_as!(Draft, r#"SELECT product_id,revision,result FROM translation_items WHERE tenant=$1 AND job_id=$2 AND status='ready' AND ($3::text IS NULL OR product_id=$3) ORDER BY product_id FOR UPDATE LIMIT 50"#, tenant, job, product)
        .fetch_all(tx)
        .await?)
}

/// The current aggregate revision and metadata of a product, locked.
pub async fn lock_product(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
) -> Result<Option<(i64, Value)>> {
    Ok(sqlx::query!(
        r#"SELECT revision,extra FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE"#,
        tenant,
        product
    )
    .fetch_optional(tx)
    .await?
    .map(|row| (row.revision, row.extra)))
}

/// Stores translated metadata and advances the aggregate revision.
pub async fn update_metadata(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    extra: Value,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE products SET extra=$1,revision=revision+1 WHERE tenant=$2 AND id=$3"#,
        extra,
        tenant,
        product
    )
    .execute(tx)
    .await?;
    Ok(())
}

pub async fn set_draft_status(
    tx: &mut PgConnection,
    tenant: &str,
    job: &str,
    product: &str,
    status: &str,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE translation_items SET status=$1 WHERE tenant=$2 AND job_id=$3 AND product_id=$4"#,
        status,
        tenant,
        job,
        product
    )
    .execute(tx)
    .await?;
    Ok(())
}

pub async fn ready_drafts(tx: &mut PgConnection, tenant: &str, job: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM translation_items WHERE tenant=$1 AND job_id=$2 AND status='ready'"#, tenant, job)
    .fetch_one(tx)
    .await?)
}

/// How many products a new job covers and the highest product ID it will reach.
pub async fn selection_size(
    tx: &mut PgConnection,
    tenant: &str,
    products: Option<&[String]>,
) -> Result<(i64, String)> {
    let row = sqlx::query!(r#"SELECT count(*) AS "total!",coalesce(max(id),'') AS "highwater!" FROM products WHERE tenant=$1 AND ($2::text[] IS NULL OR id=ANY($2))"#, tenant, products)
    .fetch_one(tx)
    .await?;
    Ok((row.total, row.highwater))
}

/// A translation job to enqueue.
pub struct NewJob<'a> {
    pub id: &'a str,
    pub source_locale: &'a str,
    pub target_locale: &'a str,
    pub choice: &'a Value,
    pub overwrite: bool,
    pub total: i64,
    pub highwater: String,
    pub products: Option<&'a [String]>,
}

pub async fn insert(tx: &mut PgConnection, tenant: &str, job: NewJob<'_>) -> Result<()> {
    sqlx::query!(r#"INSERT INTO translation_jobs(tenant,id,source_locale,target_locale,choice,overwrite,total,highwater,product_ids) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)"#, tenant, job.id, job.source_locale, job.target_locale, job.choice, job.overwrite, job.total, job.highwater, job.products)
        .execute(tx)
        .await?;
    Ok(())
}

/// The latest 30 jobs as documents without tenant and lease columns.
pub async fn latest(a: &State, tenant: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' AS "job!" FROM translation_jobs j WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30"#, tenant)
        .fetch_all(&a.db)
        .await?)
}

/// One job as a document without tenant and lease columns.
pub async fn document(a: &State, tenant: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' AS "job!" FROM translation_jobs j WHERE tenant=$1 AND id=$2"#, tenant, id)
        .fetch_optional(&a.db)
        .await?)
}

/// Up to 51 drafts of a job after the product cursor, as documents.
pub async fn draft_page(a: &State, tenant: &str, job: &str, cursor: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(i)-'tenant'-'job_id' AS "draft!" FROM translation_items i WHERE tenant=$1 AND job_id=$2 AND product_id>$3 ORDER BY product_id LIMIT 51"#, tenant, job, cursor)
        .fetch_all(&a.db)
        .await?)
}

/// The number of drafts per status, as a JSON object.
pub async fn status_counts(a: &State, tenant: &str, job: &str) -> Result<Value> {
    Ok(sqlx::query_scalar!(r#"SELECT coalesce(jsonb_object_agg(status,n),'{}') AS "counts!" FROM (SELECT status,count(*) AS n FROM translation_items WHERE tenant=$1 AND job_id=$2 GROUP BY status) s"#, tenant, job)
        .fetch_one(&a.db)
        .await?)
}

/// Sets the status of a job no task controls and clears its lease; `false` when no such open job exists.
pub async fn control(a: &State, tenant: &str, id: &str, status: &str) -> Result<bool> {
    Ok(sqlx::query!(r#"UPDATE translation_jobs SET status=$1,error=NULL,lease=NULL,lease_until=NULL WHERE tenant=$2 AND id=$3 AND run_task IS NULL AND status IN ('queued','failed','processing')"#, status, tenant, id)
        .execute(&a.db)
        .await?
        .rows_affected()
        > 0)
}
