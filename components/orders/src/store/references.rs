//! Where orders refer to a delivery or payment method, for the deletion guard of the shipping component.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Orders whose snapshot at `snapshot` or checkout selection `selection` names the method; with a channel, only orders of that channel's carts.
pub async fn method_orders(
    tx: &mut PgConnection,
    tenant: &str,
    snapshot: &[&str],
    id: &str,
    channel: Option<&str>,
    selection: &str,
) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM orders o JOIN carts c ON c.id=o.cart_id WHERE o.tenant=$1 AND ($4::text IS NULL OR COALESCE(c.data->>'sales_channel','default')=$4) AND (o.data #>> $2::text[]=$3 OR o.data->'cart'->'checkout'->>$5=$3)"#, tenant, snapshot as &[&str], id, channel, selection)
        .fetch_one(tx)
        .await?)
}
