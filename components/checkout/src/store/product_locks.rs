//! Product locks of the purchase: the cart's products and their parents in id order, rechecked for channel visibility under the lock.
use sqlx::PgConnection;
use sqlx::postgres::PgRow;
use veyndo_core::Result;

/// The distinct parents of the given products.
pub async fn parents(tx: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT parent_id AS "parent_id!" FROM products WHERE tenant=$1 AND id=ANY($2) AND parent_id IS NOT NULL"#,
        tenant,
        ids
    )
    .fetch_all(tx)
    .await?)
}

/// The complete product rows, row-locked in id order.
pub async fn lock(tx: &mut PgConnection, tenant: &str, ids: &[String]) -> Result<Vec<PgRow>> {
    // dynamic SQL: complete product rows for the catalog's shared row mapper (veyndo_catalog::store::products::product), whose columns the catalog owns.
    Ok(
        sqlx::query("SELECT * FROM products WHERE tenant=$1 AND id=ANY($2) ORDER BY id FOR UPDATE")
            .bind(tenant)
            .bind(ids)
            .fetch_all(tx)
            .await?,
    )
}

/// Any of the products is hidden in the channel.
pub async fn hidden(
    tx: &mut PgConnection,
    tenant: &str,
    ids: &[String],
    channel: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM product_channel_visibility WHERE tenant=$1 AND product_id=ANY($2) AND channel_id=$3 AND NOT visible) AS "hidden!""#,
        tenant,
        ids,
        channel
    )
    .fetch_one(tx)
    .await?)
}
