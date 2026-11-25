//! Where open carts refer to a method or a currency, for the guards of the shipping and pricing components.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Open carts that selected the method under `selection` or the older key `legacy`; with a channel, only that channel's carts.
pub async fn method_carts(
    tx: &mut PgConnection,
    tenant: &str,
    selection: &str,
    id: &str,
    channel: Option<&str>,
    legacy: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM carts WHERE tenant=$1 AND status='open' AND ($4::text IS NULL OR COALESCE(data->>'sales_channel','default')=$4) AND (data->'checkout'->>$2=$3 OR data->'checkout'->>$5=$3)"#, tenant, selection, id, channel, legacy)
        .fetch_one(tx)
        .await?)
}

/// Whether an open cart is priced in one of the currencies.
pub async fn currency_carts(tx: &mut PgConnection, tenant: &str, codes: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM carts WHERE tenant=$1 AND status='open' AND data->>'currency'=ANY($2)) AS "used!""#, tenant, codes)
        .fetch_one(tx)
        .await?)
}
