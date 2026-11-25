//! Outbox runtime SQL: tenant counters and the explicit release of one quarantined event that was neither delivered nor retired.
use veyndo_core::{Result, State};

/// Pending, quarantined, retrying and consumed event counts of one shop.
pub async fn counters(a: &State, tenant: &str) -> Result<[i64; 4]> {
    let r=sqlx::query!(r#"SELECT (SELECT count(*) FROM outbox WHERE tenant=$1 AND delivered_at IS NULL AND dead_letter_at IS NULL) AS "pending!",(SELECT count(*) FROM outbox WHERE tenant=$1 AND dead_letter_at IS NOT NULL) AS "quarantined!",(SELECT count(*) FROM outbox WHERE tenant=$1 AND delivered_at IS NULL AND attempts>0 AND dead_letter_at IS NULL) AS "retrying!",(SELECT count(*) FROM projections WHERE tenant=$1) AS "consumed!""#, tenant).fetch_one(&a.db).await?;
    Ok([r.pending, r.quarantined, r.retrying, r.consumed])
}

/// Releases a quarantined event and its quarantined subscriber lanes; false when nothing in this shop was quarantined.
pub async fn release(a: &State, tenant: &str, id: i64) -> Result<bool> {
    let mut tx = a.db.begin().await?;
    let changed=sqlx::query!("UPDATE outbox_subscriber_failures c SET attempts=0,available_at=now(),dead_letter_at=NULL WHERE c.tenant=$1 AND c.event_id=$2 AND c.dead_letter_at IS NOT NULL AND EXISTS(SELECT 1 FROM outbox e WHERE e.id=$2 AND e.tenant=c.tenant AND e.delivered_at IS NULL AND e.payload_retired_at IS NULL)", tenant, id).execute(&mut *tx).await?.rows_affected();
    let released=sqlx::query!("UPDATE outbox SET attempts=0,available_at=now(),dispatch_not_before=now(),dispatch_requires_retry=false,dead_letter_at=NULL,error_code=NULL WHERE tenant=$1 AND id=$2 AND delivered_at IS NULL AND payload_retired_at IS NULL AND (dispatch_requires_retry OR $3)", tenant, id, changed>0).execute(&mut *tx).await?.rows_affected();
    if released == 0 {
        return Ok(false);
    }
    tx.commit().await?;
    sqlx::query!("SELECT pg_notify('veyndo_work_ready','outbox')")
        .execute(&a.db)
        .await?;
    Ok(true)
}
