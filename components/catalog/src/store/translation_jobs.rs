//! Translation jobs and their per-product drafts, as far as applying reviewed drafts needs them.
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::{Result, State};

/// How many of the given products exist in the shop.
pub async fn count_products(tx: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM products WHERE tenant=$1 AND id=ANY($2)")
            .bind(tenant)
            .bind(ids)
            .fetch_one(tx)
            .await?,
    )
}

/// The languages of a job that is ready to be applied, locked.
pub struct ReadyJob {
    pub source_locale: String,
    pub target_locale: String,
}

pub async fn lock_ready(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<ReadyJob>> {
    Ok(sqlx::query(
        "SELECT * FROM translation_jobs WHERE tenant=$1 AND id=$2 AND status='ready' FOR UPDATE",
    )
    .bind(tenant)
    .bind(id)
    .fetch_optional(tx)
    .await?
    .map(|job| ReadyJob {
        source_locale: job.get("source_locale"),
        target_locale: job.get("target_locale"),
    }))
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
    Ok(sqlx::query("SELECT * FROM translation_items WHERE tenant=$1 AND job_id=$2 AND status='ready' AND ($3::text IS NULL OR product_id=$3) ORDER BY product_id FOR UPDATE LIMIT 50")
        .bind(tenant)
        .bind(job)
        .bind(product)
        .fetch_all(tx)
        .await?
        .iter()
        .map(|item| Draft {
            product_id: item.get("product_id"),
            revision: item.get("revision"),
            result: item.get("result"),
        })
        .collect())
}

/// The current aggregate revision and metadata of a product, locked.
pub async fn lock_product(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
) -> Result<Option<(i64, Value)>> {
    Ok(
        sqlx::query("SELECT revision,extra FROM products WHERE tenant=$1 AND id=$2 FOR UPDATE")
            .bind(tenant)
            .bind(product)
            .fetch_optional(tx)
            .await?
            .map(|row| (row.get("revision"), row.get("extra"))),
    )
}

/// Stores translated metadata and advances the aggregate revision.
pub async fn update_metadata(
    tx: &mut PgConnection,
    tenant: &str,
    product: &str,
    extra: Value,
) -> Result<()> {
    sqlx::query("UPDATE products SET extra=$1,revision=revision+1 WHERE tenant=$2 AND id=$3")
        .bind(extra)
        .bind(tenant)
        .bind(product)
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
    sqlx::query(
        "UPDATE translation_items SET status=$1 WHERE tenant=$2 AND job_id=$3 AND product_id=$4",
    )
    .bind(status)
    .bind(tenant)
    .bind(job)
    .bind(product)
    .execute(tx)
    .await?;
    Ok(())
}

pub async fn ready_drafts(tx: &mut PgConnection, tenant: &str, job: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM translation_items WHERE tenant=$1 AND job_id=$2 AND status='ready'",
    )
    .bind(tenant)
    .bind(job)
    .fetch_one(tx)
    .await?)
}

/// The event payload of a product changed by an applied translation.
pub fn applied_event(product: &str, locale: &str, revision: i64, job: &str) -> Value {
    json!({"productId":product,"locale":locale,"revision":revision,"jobId":job})
}

/// How many products a new job covers and the highest product ID it will reach.
pub async fn selection_size(
    tx: &mut PgConnection,
    tenant: &str,
    products: Option<&[String]>,
) -> Result<(i64, String)> {
    let row = sqlx::query(
        "SELECT count(*) AS total,coalesce(max(id),'') AS highwater FROM products WHERE tenant=$1 AND ($2::text[] IS NULL OR id=ANY($2))",
    )
    .bind(tenant)
    .bind(products)
    .fetch_one(tx)
    .await?;
    Ok((row.get("total"), row.get("highwater")))
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
    sqlx::query("INSERT INTO translation_jobs(tenant,id,source_locale,target_locale,choice,overwrite,total,highwater,product_ids) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(tenant)
        .bind(job.id)
        .bind(job.source_locale)
        .bind(job.target_locale)
        .bind(job.choice)
        .bind(job.overwrite)
        .bind(job.total)
        .bind(job.highwater)
        .bind(job.products)
        .execute(tx)
        .await?;
    Ok(())
}

/// The latest 30 jobs as documents without tenant and lease columns.
pub async fn latest(a: &State, tenant: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' FROM translation_jobs j WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30")
        .bind(tenant)
        .fetch_all(&a.db)
        .await?)
}

/// One job as a document without tenant and lease columns.
pub async fn document(a: &State, tenant: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(j)-'tenant'-'lease'-'lease_until' FROM translation_jobs j WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?)
}

/// Up to 51 drafts of a job after the product cursor, as documents.
pub async fn draft_page(a: &State, tenant: &str, job: &str, cursor: &str) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT to_jsonb(i)-'tenant'-'job_id' FROM translation_items i WHERE tenant=$1 AND job_id=$2 AND product_id>$3 ORDER BY product_id LIMIT 51")
        .bind(tenant)
        .bind(job)
        .bind(cursor)
        .fetch_all(&a.db)
        .await?)
}

/// The number of drafts per status, as a JSON object.
pub async fn status_counts(a: &State, tenant: &str, job: &str) -> Result<Value> {
    Ok(sqlx::query_scalar("SELECT coalesce(jsonb_object_agg(status,n),'{}') FROM (SELECT status,count(*) AS n FROM translation_items WHERE tenant=$1 AND job_id=$2 GROUP BY status) s")
        .bind(tenant)
        .bind(job)
        .fetch_one(&a.db)
        .await?)
}

/// Sets the status of a job no task controls and clears its lease; `false` when no such open job exists.
pub async fn control(a: &State, tenant: &str, id: &str, status: &str) -> Result<bool> {
    Ok(sqlx::query("UPDATE translation_jobs SET status=$1,error=NULL,lease=NULL,lease_until=NULL WHERE tenant=$2 AND id=$3 AND run_task IS NULL AND status IN ('queued','failed','processing')")
        .bind(status)
        .bind(tenant)
        .bind(id)
        .execute(&a.db)
        .await?
        .rows_affected()
        > 0)
}
