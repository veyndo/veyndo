//! Receipt claims: one provider receipt belongs to exactly one attempt, across tenants.
use crate::domain::attempt::Attempt;
use veyndo_core::{Result, conflict};

pub async fn claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    kind: &str,
    id: &str,
    key: &str,
) -> Result<()> {
    sqlx::query!("INSERT INTO payment_receipt_claims(provider,environment,account_ref,kind,receipt_ref,tenant,attempt_id,job_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT DO NOTHING", p.provider, p.environment, p.context["accountRef"].as_str().unwrap_or(""), kind, id, p.tenant, p.id, key).execute(&mut **tx).await?;
    let row=sqlx::query!("SELECT tenant,attempt_id,job_key FROM payment_receipt_claims WHERE provider=$1 AND environment=$2 AND account_ref=$3 AND kind=$4 AND receipt_ref=$5 FOR UPDATE", p.provider, p.environment, p.context["accountRef"].as_str().unwrap_or(""), kind, id).fetch_one(&mut **tx).await?;
    if row.tenant != p.tenant
        || row.attempt_id != p.id
        || (["refund", "capture-job"].contains(&kind) && row.job_key != key)
    {
        return Err(conflict("Provider receipt already allocated"));
    }
    Ok(())
}
