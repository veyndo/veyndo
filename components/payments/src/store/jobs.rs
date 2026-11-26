//! Durable payment jobs: idempotent admission (one key, one fingerprint), the worker's lease, fence and outcome writes, all inside the caller's transaction. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
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
    sqlx::query!("INSERT INTO payment_jobs(id,tenant,attempt_id,operation,request,fingerprint) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING", job, t, id, op, v, fingerprint).execute(&mut **tx).await?;
    let old: String = sqlx::query_scalar!(
        "SELECT fingerprint FROM payment_jobs WHERE tenant=$1 AND id=$2",
        t,
        job
    )
    .fetch_one(&mut **tx)
    .await?;
    if old != fingerprint {
        return Err(conflict(
            "Payment idempotency key used for a different operation",
        ));
    }
    Ok(job)
}

/// Records a provider event once; the count is 0 for a repeated event.
pub async fn record_inbox(
    tx: &mut PgConnection,
    (tenant, provider, event): (&str, &str, &str),
    data: &Value,
) -> Result<u64> {
    Ok(sqlx::query!("INSERT INTO payment_inbox(tenant,provider,event_id,data) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING", tenant, provider, event, data).execute(tx).await?.rows_affected())
}

/// A job as the merchant polls it.
pub struct JobStatus {
    pub id: String,
    pub state: String,
    pub error: Option<String>,
    pub attempt_id: String,
}

/// One job of the shop.
pub async fn status<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    id: &str,
) -> Result<Option<JobStatus>> {
    Ok(sqlx::query_as!(
        JobStatus,
        "SELECT id,state,error,attempt_id FROM payment_jobs WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// A due job the worker claims.
pub struct Due {
    pub id: String,
    pub attempt_id: String,
    pub operation: String,
    pub request: Value,
    pub attempts: i32,
}

/// The oldest due job of a locked attempt, locked and skipped by other workers.
pub async fn lock_due(tx: &mut PgConnection, attempt: &str) -> Result<Option<Due>> {
    Ok(sqlx::query_as!(
        Due,
        "SELECT id,attempt_id,operation,request,attempts FROM payment_jobs WHERE attempt_id=$1 AND available_at<=now() AND (state='queued' OR state='running' AND lease_until<now()) ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED",
        attempt
    )
    .fetch_optional(tx)
    .await?)
}

/// Leases a claimed job for sixty seconds under its new generation.
pub async fn lease(tx: &mut PgConnection, id: &str, generation: i32) -> Result<()> {
    sqlx::query!("UPDATE payment_jobs SET state='running',attempts=$1,lease_until=now()+interval '60 seconds' WHERE id=$2", generation, id).execute(tx).await?;
    Ok(())
}

/// The lease fence of a job: its state and generation, row-locked.
pub struct Fence {
    pub state: String,
    pub attempts: i32,
}

/// Locks a job and reads its fence.
pub async fn lock_fence(tx: &mut PgConnection, id: &str) -> Result<Fence> {
    Ok(sqlx::query_as!(
        Fence,
        "SELECT state,attempts FROM payment_jobs WHERE id=$1 FOR UPDATE",
        id
    )
    .fetch_one(tx)
    .await?)
}

/// Records a provider refund that is still pending; a repeated receipt is ignored.
pub async fn record_pending_refund(
    tx: &mut PgConnection,
    (tenant, job, attempt): (&str, &str, &str),
    remote: &str,
    amount: i64,
) -> Result<()> {
    sqlx::query!("INSERT INTO payment_refunds(tenant,job_id,attempt_id,provider_id,status,amount_minor) VALUES($1,$2,$3,$4,'pending',$5) ON CONFLICT DO NOTHING", tenant, job, attempt, remote, amount).execute(tx).await?;
    Ok(())
}

/// Puts a job back in the queue for another try in ten seconds, releasing its lease.
pub async fn requeue(tx: &mut PgConnection, id: &str) -> Result<()> {
    sqlx::query!("UPDATE payment_jobs SET state='queued',available_at=now()+interval '10 seconds',lease_until=NULL WHERE id=$1", id).execute(tx).await?;
    Ok(())
}

/// Opens the savepoint that keeps a rejected remote receipt from partially mutating local state.
pub async fn begin_receipt(tx: &mut PgConnection) -> Result<()> {
    sqlx::query!("SAVEPOINT receipt").execute(tx).await?;
    Ok(())
}

/// Discards everything a rejected receipt wrote, keeping the claim.
pub async fn discard_receipt(tx: &mut PgConnection) -> Result<()> {
    sqlx::query!("ROLLBACK TO SAVEPOINT receipt")
        .execute(tx)
        .await?;
    Ok(())
}

/// Marks a job uncertain with the provider's error; it needs reconciliation.
pub async fn uncertain(tx: &mut PgConnection, id: &str, error: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE payment_jobs SET state='uncertain',error=$1,lease_until=NULL WHERE id=$2",
        error,
        id
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Marks a job succeeded with the provider's response.
pub async fn succeeded(tx: &mut PgConnection, id: &str, response: &Value) -> Result<()> {
    sqlx::query!("UPDATE payment_jobs SET state='succeeded',response=$1,error=NULL,lease_until=NULL WHERE id=$2", response, id).execute(tx).await?;
    Ok(())
}

/// Records a failed dispatch: `state` is `queued` for another try in two seconds, or the terminal `failed`/`uncertain`.
pub async fn failed(tx: &mut PgConnection, id: &str, state: &str, error: &str) -> Result<()> {
    sqlx::query!("UPDATE payment_jobs SET state=$1,error=$2,lease_until=NULL,available_at=now()+interval '2 seconds' WHERE id=$3", state, error, id).execute(tx).await?;
    Ok(())
}

/// A job of the shop's merchant overview.
pub struct Listed {
    pub id: String,
    pub attempt_id: String,
    pub operation: String,
    pub state: String,
    pub error: Option<String>,
}

/// The shop's latest 100 jobs, newest first.
pub async fn latest<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
) -> Result<Vec<Listed>> {
    Ok(sqlx::query_as!(
        Listed,
        "SELECT id,attempt_id,operation,state,error FROM payment_jobs WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100",
        tenant
    )
    .fetch_all(db)
    .await?)
}

/// A job of one attempt in the payment details.
pub struct OfAttempt {
    pub id: String,
    pub operation: String,
    pub state: String,
    pub created_at: String,
    pub attempts: i32,
    pub error: Option<String>,
}

/// The latest 100 jobs of one attempt, newest first.
pub async fn of_attempt<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    attempt: &str,
) -> Result<Vec<OfAttempt>> {
    Ok(sqlx::query_as!(
        OfAttempt,
        r#"SELECT id,operation,state,to_jsonb(created_at)#>>'{}' AS "created_at!",attempts,error FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 ORDER BY created_at DESC,id DESC LIMIT 100"#,
        tenant,
        attempt
    )
    .fetch_all(db)
    .await?)
}
