//! Where products refer to a currency or how far the product IDs reach, for the pricing component.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Whether a product carries fixed prices in, or is priced in, one of the currencies.
pub async fn currency_products(
    tx: &mut PgConnection,
    tenant: &str,
    codes: &[String],
) -> Result<bool> {
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM products WHERE tenant=$1 AND ((extra->'currencyPrices') ?| $2 OR extra->>'priceCurrency'=ANY($2))) AS "used!""#, tenant, codes)
        .fetch_one(tx)
        .await?)
}

/// The highest product ID of the shop, or an empty string.
pub async fn highest_id(tx: &mut PgConnection, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(
        r#"SELECT coalesce(max(id),'') AS "ceiling!" FROM products WHERE tenant=$1"#,
        tenant
    )
    .fetch_one(tx)
    .await?)
}
