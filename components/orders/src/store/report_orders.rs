//! Order facts of merchant reports: the first order day in a time zone and the currencies orders were placed in, read inside the report's snapshot transaction.
use veyndo_core::{Result, State};

/// The local date of the shop's first order in the time zone, as `YYYY-MM-DD`.
pub async fn first_day(a: &State, t: &str, timezone: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT (min(created_at) AT TIME ZONE $2)::date::text FROM orders WHERE tenant=$1",
        t,
        timezone
    )
    .fetch_one(&a.db)
    .await?)
}

/// Up to 100 distinct currencies orders were placed in.
pub async fn currencies(c: &mut sqlx::PgConnection, t: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(r#"SELECT DISTINCT coalesce(data#>>'{money,currency,code}',data#>>'{cart,price,currency}','EUR') AS "currency!" FROM orders WHERE tenant=$1 ORDER BY 1 LIMIT 100"#, t).fetch_all(&mut *c).await?)
}
