//! Where orders refer to a delivery or payment method, and the order counts, amounts, timelines, recent documents and receipt identity the shop component's operator and settings views show.
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

/// Orders of the last `days` days of all shops outside `excluded`.
pub async fn count_outside(conn: &mut PgConnection, days: i64, excluded: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM orders o WHERE created_at>=current_date-($1::integer-1)*interval '1 day' AND NOT (tenant=ANY($2))"#,
        days as i32,
        excluded
    )
    .fetch_one(conn)
    .await?)
}

/// Orders of the last `days` days per shop among `tenants`; shops without orders are left out.
pub async fn count_by_shop(
    a: &veyndo_core::State,
    tenants: &[String],
    days: i64,
) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM orders WHERE tenant=ANY($1) AND created_at>=current_date-($2::integer-1)*interval '1 day' GROUP BY tenant"#,
        tenants,
        days as i32
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}

/// The amounts per currency of all shops outside `excluded`, or of `tenant`, for a window of `days`.
pub async fn amounts(
    conn: &mut PgConnection,
    days: i64,
    tenant: Option<&str>,
    excluded: &[String],
) -> Result<Vec<veyndo_shop::store::operator_metrics::Amounts>> {
    Ok(sqlx::query_as!(
        veyndo_shop::store::operator_metrics::Amounts,
        r#"SELECT coalesce(data->'cart'->'price'->>'currency','EUR') AS "currency!", count(*) AS "orders!", coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric) FILTER(WHERE data->>'state' NOT IN ('cancelled','canceled')),0)::text AS "booked!", coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric) FILTER(WHERE data->'payment'->>'provider'='simulated' AND data->>'state' NOT IN ('cancelled','canceled')),0)::text AS "simulated!", coalesce(sum(CASE WHEN data#>>'{payment,capturedMinor}' IS NOT NULL THEN (data#>>'{payment,capturedMinor}')::numeric/power(10::numeric,coalesce((data#>>'{cart,price,currencyScale}')::int,2)) ELSE (data->'cart'->'price'->>'totalPrice')::numeric END) FILTER(WHERE data->'payment'->>'realMoneyCharged'='true' AND data->'payment'->>'state' IN ('captured','partially_captured','captured_late','partially_refunded','refunded')),0)::text AS "captured!" FROM orders o WHERE created_at>=current_date-($1::integer-1)*interval '1 day' AND NOT (tenant=ANY($3)) AND ($2::text IS NULL OR tenant=$2) GROUP BY 1 ORDER BY 1"#,
        days as i32,
        tenant,
        excluded
    )
    .fetch_all(conn)
    .await?)
}

/// The orders of a shop per day for the last `days` days, oldest first.
pub async fn timeline(
    a: &veyndo_core::State,
    tenant: &str,
    days: i64,
) -> Result<Vec<veyndo_shop::store::operator_metrics::Day>> {
    Ok(sqlx::query_as!(
        veyndo_shop::store::operator_metrics::Day,
        r#"SELECT d::date::text AS "day!",count(o.id) AS "orders!" FROM generate_series(current_date-($2::integer-1)*interval '1 day',current_date,interval '1 day') d LEFT JOIN orders o ON o.tenant=$1 AND o.created_at>=d AND o.created_at<d+interval '1 day' GROUP BY d ORDER BY d"#,
        tenant,
        days as i32
    )
    .fetch_all(&a.db)
    .await?)
}

/// The newest thirty order documents without their cart token.
pub async fn recent(a: &veyndo_core::State, t: &str) -> Result<Vec<serde_json::Value>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT data #- '{cart,token}' AS "data!" FROM orders WHERE tenant=$1 ORDER BY created_at DESC LIMIT 30"#,
        t
    )
    .fetch_all(&a.db)
    .await?)
}

/// The validated business identity of a shop's receipts.
pub async fn receipt_identity(
    a: &veyndo_core::State,
    tenant: &str,
) -> Result<Option<serde_json::Value>> {
    Ok(
        sqlx::query_scalar!("SELECT data FROM receipt_settings WHERE tenant=$1", tenant)
            .fetch_optional(&a.db)
            .await?,
    )
}
