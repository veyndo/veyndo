//! Sign-in attempt buckets shared by all replicas: one counted attempt per call and an escalating block after failures. Columns checked at compile time.
use veyndo_core::{Result, State};

/// Counts one attempt in the bucket `key`; `None` when the bucket is full for this minute or blocked.
pub async fn reserve(a: &State, key: &str, limit: i32) -> Result<Option<i32>> {
    Ok(sqlx::query_scalar!(
        r#"INSERT INTO auth_attempt_buckets(key,attempts) VALUES($1,1) ON CONFLICT(key) DO UPDATE SET attempts=CASE WHEN auth_attempt_buckets.window_at < now()-interval '1 minute' THEN 1 ELSE auth_attempt_buckets.attempts+1 END,window_at=CASE WHEN auth_attempt_buckets.window_at < now()-interval '1 minute' THEN now() ELSE auth_attempt_buckets.window_at END,updated_at=now() WHERE auth_attempt_buckets.blocked_until<=now() AND (auth_attempt_buckets.window_at<now()-interval '1 minute' OR auth_attempt_buckets.attempts<$2) RETURNING attempts"#,
        key,
        limit
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Blocks a bucket with at least five attempts for 2^attempts seconds (at most a minute) after a refused sign-in.
pub async fn block(a: &State, key: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE auth_attempt_buckets SET blocked_until=now()+least(60,power(2,least(attempts,6))::int)*interval '1 second' WHERE key=$1 AND attempts>=5"#,
        key
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
