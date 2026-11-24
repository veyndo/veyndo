//! Durable idempotent payment commands, customer context binding and serial refund admission.
use crate::domain::settlements;
use crate::store::attempts::attempt;
use crate::store::jobs::enqueue_tx;
use crate::store::{capture_admission, refund_allocation};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{tenant, token};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_kernel::verified_kernel;

/// The JSON fields `enqueue` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EnqueueInput {
    #[serde(deserialize_with = "lenient")]
    amount_minor: Option<i64>,
}

pub async fn enqueue(
    a: &State,
    h: &RequestContext,
    id: &str,
    op: &str,
    key: &str,
    v: &Value,
) -> Result<Value> {
    let input: EnqueueInput = view(v);
    let t = tenant(h)?;
    let mut tx = a.db.begin().await?;
    let r = sqlx::query("SELECT * FROM payment_attempts WHERE tenant=$1 AND id=$2 FOR UPDATE")
        .bind(&t)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Payment not found".into()))?;
    let p = attempt(&r);
    if p.provider != "paypal"
        && ["capture", "authorize", "void", "refund"].contains(&op)
        && !p.context["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|c| c == op))
    {
        return Err(bad("Payment operation is not supported by this method"));
    }
    // Existing operation retries return the original job, even after its terminal state.
    let job = hash(&format!("{t}:{key}"));
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payment_jobs WHERE tenant=$1 AND id=$2)")
            .bind(&t)
            .bind(&job)
            .fetch_one(&mut *tx)
            .await?;
    if !exists {
        if op == "capture" {
            capture_admission::validate(&mut tx, &p, v).await?;
        } else if op == "refund" {
            let amount = input
                .amount_minor
                .filter(|v| *v > 0)
                .ok_or(bad("Positive amountMinor required"))?;
            let reserved:i64=sqlx::query_scalar("SELECT coalesce(sum((request->>'amountMinor')::bigint),0)::bigint FROM payment_jobs WHERE tenant=$1 AND attempt_id=$2 AND operation='refund' AND state IN ('queued','running','uncertain')").bind(&t).bind(id).fetch_one(&mut *tx).await?;
            if ![
                "captured",
                "captured_late",
                "partially_refunded",
                "partially_captured",
            ]
            .contains(&p.state.as_str())
                || !p
                    .refunded
                    .checked_add(reserved)
                    .and_then(|consumed| {
                        Some((
                            u64::try_from(settlements::captured(&p)).ok()?,
                            u64::try_from(consumed).ok()?,
                        ))
                    })
                    .is_some_and(|(captured, consumed)| {
                        verified_kernel::refund_admissible(captured, consumed, amount as u64)
                    })
            {
                return Err(conflict("Refund exceeds the remaining captured amount"));
            }
            refund_allocation::validate(&mut tx, &p, v).await?;
        } else if op == "void" && settlements::multi(&p) && !settlements::closed(&p) {
            // Voiding the uncaptured remainder never releases already sold inventory.
        } else if !["pending", "ready", "approved", "authorized"].contains(&p.state.as_str())
            && op != "reconcile"
        {
            return Err(conflict("Payment is terminal"));
        }
    }
    let job = enqueue_tx(&mut tx, &t, id, op, key, v).await?;
    tx.commit().await?;
    Ok(json!({"jobId":job,"queued":true}))
}
pub async fn customer(a: &State, h: &RequestContext, id: &str) -> Result<()> {
    let ok:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payment_attempts p JOIN orders o ON o.id=p.order_id AND o.tenant=p.tenant JOIN carts c ON c.id=o.cart_id AND c.tenant=o.tenant WHERE p.tenant=$1 AND p.id=$2 AND c.token=$3)").bind(tenant(h)?).bind(id).bind(token(h)?).fetch_one(&a.db).await?;
    if !ok {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Payment not found in this customer context".into(),
        ));
    }
    Ok(())
}
