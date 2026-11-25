//! Aggregate control-plane reads across live shops (sandboxes excluded): totals, amounts per currency, shop pages and daily order timelines. Columns checked at compile time.
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// Platform totals for a window of `days`, with the time they were read.
pub struct Totals {
    pub shops: i64,
    pub sandboxes: i64,
    pub users: i64,
    pub products: i64,
    pub customers: i64,
    pub orders: i64,
    pub pending_events: i64,
    pub generated: String,
}

/// The platform totals inside the caller's read snapshot.
pub async fn totals(conn: &mut PgConnection, days: i64) -> Result<Totals> {
    Ok(sqlx::query_as!(
        Totals,
        r#"SELECT (SELECT count(*) FROM tenants t WHERE NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id)) AS "shops!",(SELECT count(*) FROM shop_environments) AS "sandboxes!",(SELECT count(*) FROM merchant_users) AS "users!",(SELECT count(*) FROM products p WHERE NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=p.tenant)) AS "products!",(SELECT count(*) FROM customers c WHERE NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=c.tenant)) AS "customers!",(SELECT count(*) FROM orders o WHERE created_at>=current_date-($1::integer-1)*interval '1 day' AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=o.tenant)) AS "orders!",(SELECT count(*) FROM outbox o WHERE delivered_at IS NULL AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=o.tenant)) AS "pending_events!",now()::text AS "generated!""#,
        days as i32
    )
    .fetch_one(conn)
    .await?)
}

/// Recorded, simulated and confirmed captured amounts of one currency, as decimal text.
pub struct Amounts {
    pub currency: String,
    pub orders: i64,
    pub booked: String,
    pub simulated: String,
    pub captured: String,
}

/// The amounts per currency of all live shops, or of `tenant`, for a window of `days`.
pub async fn amounts<'e>(
    db: impl sqlx::PgExecutor<'e>,
    days: i64,
    tenant: Option<&str>,
) -> Result<Vec<Amounts>> {
    Ok(sqlx::query_as!(
        Amounts,
        r#"SELECT coalesce(data->'cart'->'price'->>'currency','EUR') AS "currency!", count(*) AS "orders!", coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric) FILTER(WHERE data->>'state' NOT IN ('cancelled','canceled')),0)::text AS "booked!", coalesce(sum((data->'cart'->'price'->>'totalPrice')::numeric) FILTER(WHERE data->'payment'->>'provider'='simulated' AND data->>'state' NOT IN ('cancelled','canceled')),0)::text AS "simulated!", coalesce(sum(CASE WHEN data#>>'{payment,capturedMinor}' IS NOT NULL THEN (data#>>'{payment,capturedMinor}')::numeric/power(10::numeric,coalesce((data#>>'{cart,price,currencyScale}')::int,2)) ELSE (data->'cart'->'price'->>'totalPrice')::numeric END) FILTER(WHERE data->'payment'->>'realMoneyCharged'='true' AND data->'payment'->>'state' IN ('captured','partially_captured','captured_late','partially_refunded','refunded')),0)::text AS "captured!" FROM orders o WHERE created_at>=current_date-($1::integer-1)*interval '1 day' AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=o.tenant) AND ($2::text IS NULL OR tenant=$2) GROUP BY 1 ORDER BY 1"#,
        days as i32,
        tenant
    )
    .fetch_all(db)
    .await?)
}

/// One live shop of the operator list with its counts.
pub struct ShopRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub status_revision: i64,
    pub created: String,
    pub products: i64,
    pub customers: i64,
    pub members: i64,
    pub orders: i64,
    pub apps: i64,
    pub channels: i64,
    pub sources: i64,
}

/// Up to `limit` live shops after the cursor whose name or ID contains `search`.
pub async fn shops(
    a: &State,
    after: &str,
    search: &str,
    limit: i64,
    days: i64,
) -> Result<Vec<ShopRow>> {
    Ok(sqlx::query_as!(
        ShopRow,
        r#"SELECT t.id,t.name,t.status,t.status_revision,t.created_at::text AS "created!",(SELECT count(*) FROM products WHERE tenant=t.id) AS "products!",(SELECT count(*) FROM customers WHERE tenant=t.id) AS "customers!",(SELECT count(*) FROM memberships WHERE tenant=t.id AND active) AS "members!",(SELECT count(*) FROM orders WHERE tenant=t.id AND created_at>=current_date-($4::integer-1)*interval '1 day') AS "orders!",(SELECT count(*) FROM app_packages WHERE tenant=t.id AND active) AS "apps!",(SELECT count(*) FROM sales_channels WHERE tenant=t.id AND coalesce((data->>'active')::boolean,true)) AS "channels!",(SELECT count(*) FROM app_evidence WHERE tenant=t.id) AS "sources!" FROM tenants t WHERE t.id>$1 AND (strpos(lower(t.name),lower($2))>0 OR strpos(t.id,lower($2))>0) AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id) ORDER BY t.id LIMIT $3"#,
        after,
        search,
        limit,
        days as i32
    )
    .fetch_all(&a.db)
    .await?)
}

/// The name of a live shop; none for an unknown shop or a sandbox.
pub async fn live_name(a: &State, id: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT name FROM tenants t WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM shop_environments e WHERE e.tenant=t.id)",
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// The order count of one day.
pub struct Day {
    pub day: String,
    pub orders: i64,
}

/// The orders of a shop per day for the last `days` days, oldest first.
pub async fn timeline(a: &State, id: &str, days: i64) -> Result<Vec<Day>> {
    Ok(sqlx::query_as!(
        Day,
        r#"SELECT d::date::text AS "day!",count(o.id) AS "orders!" FROM generate_series(current_date-($2::integer-1)*interval '1 day',current_date,interval '1 day') d LEFT JOIN orders o ON o.tenant=$1 AND o.created_at>=d AND o.created_at<d+interval '1 day' GROUP BY d ORDER BY d"#,
        id,
        days as i32
    )
    .fetch_all(&a.db)
    .await?)
}
