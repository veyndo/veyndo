//! Bounded retirement of duplicate delivered payloads; provenance IDs and unsettled app/flow work survive.
use crate::{Result, State};

/// True at most once a minute per process; the caller runs its retirement work only then.
pub fn due() -> bool {
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let last = LAST.load(std::sync::atomic::Ordering::Relaxed);
    if now.saturating_sub(last) < 60
        || LAST
            .compare_exchange(
                last,
                now,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
    {
        return false;
    }
    true
}

pub async fn retire(a: &State, payload_days: i32, metadata_days: i32) -> Result<()> {
    sqlx::query("WITH old AS (SELECT id FROM outbox e WHERE delivered_at < now()-$1*interval '1 day' AND payload_retired_at IS NULL AND NOT EXISTS(SELECT 1 FROM app_deliveries d WHERE d.event_id=e.id AND d.state NOT IN ('delivered','failed','cancelled')) AND NOT EXISTS(SELECT 1 FROM flow_jobs f WHERE f.event_id=e.id AND f.state NOT IN ('completed','failed','cancelled')) ORDER BY delivered_at,id LIMIT 100 FOR UPDATE SKIP LOCKED), retired AS (UPDATE outbox SET data=jsonb_build_object('retired',true),payload_retired_at=now() WHERE id IN(SELECT id FROM old) RETURNING id) DELETE FROM projections WHERE event_id IN(SELECT id FROM retired)")
        .bind(payload_days).execute(&a.db).await?;
    // Metadata can expire only when no commerce/knowledge/idempotency record still references it.
    // PostgreSQL foreign keys remain the final guard; never cascade away evidence or uncertain work.
    sqlx::query(include_str!("retention.sql"))
        .bind(metadata_days)
        .execute(&a.db)
        .await?;
    Ok(())
}
