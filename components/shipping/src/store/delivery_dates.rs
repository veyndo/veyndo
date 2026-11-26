//! Calendar delivery windows by the database's date, so every replica agrees on today.
use sqlx::PgConnection;
use veyndo_core::Result;

/// The dates `min` and `max` calendar days from today, as ISO dates.
pub async fn window(conn: &mut PgConnection, min: i32, max: i32) -> Result<(String, String)> {
    let r = sqlx::query!(r#"SELECT (current_date+$1::int)::text AS "earliest!",(current_date+$2::int)::text AS "latest!""#, min, max)
    .fetch_one(conn)
    .await?;
    Ok((r.earliest, r.latest))
}
