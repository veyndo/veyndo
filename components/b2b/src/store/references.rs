//! Where purchasing units refer to a method or a currency, for the guards of the shipping and pricing components.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Units whose list `allowed` contains the method.
pub async fn method_units(
    tx: &mut PgConnection,
    tenant: &str,
    allowed: &str,
    id: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM b2b_units WHERE tenant=$1 AND COALESCE(data->$2,'[]'::jsonb) ? $3"#, tenant, allowed, id)
        .fetch_one(tx)
        .await?)
}

/// Whether a unit buys in one of the currencies.
pub async fn currency_units(tx: &mut PgConnection, tenant: &str, codes: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM b2b_units WHERE tenant=$1 AND data->>'currency'=ANY($2)) AS "used!""#, tenant, codes)
        .fetch_one(tx)
        .await?)
}
