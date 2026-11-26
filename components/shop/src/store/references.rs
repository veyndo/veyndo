//! Where channel settings overrides refer to a delivery or payment method.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Overrides with a patch at `area`/`id`.
pub async fn method_overrides(
    tx: &mut PgConnection,
    tenant: &str,
    area: &str,
    id: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM commerce_overrides WHERE tenant=$1 AND EXISTS(SELECT 1 FROM jsonb_array_elements(data) p WHERE p->'path'->>0=$2 AND p->'path'->>1=$3)"#, tenant, area, id)
        .fetch_one(tx)
        .await?)
}
