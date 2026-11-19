//! Translation jobs and their per-product drafts, as far as applying reviewed drafts needs them.
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::Result;

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
