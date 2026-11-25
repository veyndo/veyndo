//! Where a shop still refers to a currency: product prices, queued price jobs, open carts and purchasing units.
use sqlx::PgConnection;
use veyndo_core::Result;

pub async fn any_in_use(tx: &mut PgConnection, tenant: &str, codes: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND ((extra->'currencyPrices') ?| $2 OR extra->>'priceCurrency'=ANY($2))) OR EXISTS(SELECT 1 FROM currency_price_jobs WHERE tenant=$1 AND state='queued' AND data->>'currency'=ANY($2)) OR EXISTS(SELECT 1 FROM carts WHERE tenant=$1 AND status='open' AND data->>'currency'=ANY($2)) OR EXISTS(SELECT 1 FROM b2b_units WHERE tenant=$1 AND data->>'currency'=ANY($2)) AS "in_use!""#, tenant, codes)
        .fetch_one(tx)
        .await?)
}
