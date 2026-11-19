//! Reads of the price jobs of one shop.
use crate::types::price_job::PriceJob;
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
