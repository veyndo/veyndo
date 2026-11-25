//! Reads of the price jobs of one shop.
use crate::types::price_job::{PriceJob, QueuedJob};
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

pub async fn recent(a: &State, tenant: &str) -> Result<Vec<PriceJob>> {
    Ok(sqlx::query_as::<_, PriceJob>("SELECT id,state,processed,data,created_at::text AS created FROM currency_price_jobs WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30")
        .bind(tenant)
        .fetch_all(&a.db)
        .await?)
}

pub async fn find(a: &State, tenant: &str, id: &str) -> Result<Option<PriceJob>> {
    Ok(sqlx::query_as::<_, PriceJob>("SELECT id,state,processed,data,created_at::text AS created FROM currency_price_jobs WHERE tenant=$1 AND id=$2")
        .bind(tenant)
        .bind(id)
        .fetch_optional(&a.db)
        .await?)
}

pub async fn queued(tx: &mut PgConnection, tenant: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM currency_price_jobs WHERE tenant=$1 AND state='queued'",
    )
    .bind(tenant)
    .fetch_one(tx)
    .await?)
}

/// The highest product ID at creation; the job stops there, so products added later keep their prices.
pub async fn product_ceiling(tx: &mut PgConnection, tenant: &str) -> Result<String> {
    Ok(
        sqlx::query_scalar("SELECT coalesce(max(id),'') FROM products WHERE tenant=$1")
            .bind(tenant)
            .fetch_one(tx)
            .await?,
    )
}

pub async fn insert(
    tx: &mut PgConnection,
    tenant: &str,
    id: &str,
    data: Value,
    ceiling: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO currency_price_jobs(tenant,id,data,ceiling) VALUES($1,$2,$3,$4)")
        .bind(tenant)
        .bind(id)
        .bind(data)
        .bind(ceiling)
        .execute(tx)
        .await?;
    Ok(())
}

/// Claims the oldest queued job of any shop; another replica skips it while this transaction holds it.
pub async fn claim(tx: &mut PgConnection) -> Result<Option<QueuedJob>> {
    Ok(sqlx::query_as::<_, QueuedJob>("SELECT *,created_at::text AS snapshot_started FROM currency_price_jobs WHERE state='queued' ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1")
        .fetch_optional(tx)
        .await?)
}

pub async fn advance(
    tx: &mut PgConnection,
    job: &QueuedJob,
    cursor: &str,
    processed: i64,
    state: &str,
) -> Result<()> {
    sqlx::query("UPDATE currency_price_jobs SET cursor=$1,processed=processed+$2,state=$3 WHERE tenant=$4 AND id=$5")
        .bind(cursor)
        .bind(processed)
        .bind(state)
        .bind(&job.tenant)
        .bind(&job.id)
        .execute(tx)
        .await?;
    Ok(())
}

pub async fn fail(tx: &mut PgConnection, job: &QueuedJob, error: &str) -> Result<()> {
    sqlx::query("UPDATE currency_price_jobs SET state='failed',data=jsonb_set(data,'{error}',$1) WHERE tenant=$2 AND id=$3")
        .bind(serde_json::json!(error))
        .bind(&job.tenant)
        .bind(&job.id)
        .execute(tx)
        .await?;
    Ok(())
}

/// Marks the start of one batch inside the claiming transaction.
pub async fn begin_batch(tx: &mut PgConnection) -> Result<()> {
    sqlx::query!("SAVEPOINT price_batch").execute(tx).await?;
    Ok(())
}

/// Discards everything a rejected batch wrote, keeping the claim.
pub async fn discard_batch(tx: &mut PgConnection) -> Result<()> {
    sqlx::query!("ROLLBACK TO SAVEPOINT price_batch")
        .execute(tx)
        .await?;
    Ok(())
}
