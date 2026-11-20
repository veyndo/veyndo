//! Idempotent admission of a payment job: one key, one fingerprint, inside the caller's transaction.
use serde_json::Value;
use veyndo_core::{Result, bad, conflict, hash};

pub async fn enqueue_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    op: &str,
    key: &str,
    v: &Value,
) -> Result<String> {
    if key.len() < 8 || key.len() > 128 {
        return Err(bad("Idempotency-Key must contain 8..128 characters"));
    }
    let fingerprint = hash(&format!("{id}:{op}:{v}"));
    let job = hash(&format!("{t}:{key}"));
    sqlx::query("INSERT INTO payment_jobs(id,tenant,attempt_id,operation,request,fingerprint) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(&job).bind(t).bind(id).bind(op).bind(v).bind(&fingerprint).execute(&mut **tx).await?;
    let old: String =
        sqlx::query_scalar("SELECT fingerprint FROM payment_jobs WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(&job)
            .fetch_one(&mut **tx)
            .await?;
    if old != fingerprint {
        return Err(conflict(
            "Payment idempotency key used for a different operation",
        ));
    }
    Ok(job)
}
