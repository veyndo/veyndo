//! Serialized refund budgets: pending native refund jobs reserve their share of a capture.
use crate::domain::attempt::Attempt;
use crate::domain::refunds::{capture, checked_total, used};
use serde_json::Value;
use veyndo_core::{Result, bad, conflict};

pub async fn validate(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Attempt,
    v: &Value,
) -> Result<()> {
    let (id, total) = capture(p, &p.context, v)?;
    let pending: i64 = sqlx::query_scalar("SELECT coalesce(sum((request->>'amountMinor')::bigint),0)::bigint FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND operation='refund' AND state IN ('queued','running','uncertain') AND coalesce(request->>'captureId',$3)=$3")
        .bind(&p.tenant).bind(&p.id).bind(&id).fetch_one(&mut **tx).await?;
    let requested = v["amountMinor"]
        .as_i64()
        .ok_or(bad("Refund amount required"))?;
    checked_total(
        total,
        used(p, &p.context, &id)
            .checked_add(pending)
            .ok_or(conflict("Refund reservation overflow"))?,
        requested,
    )?;
    Ok(())
}
