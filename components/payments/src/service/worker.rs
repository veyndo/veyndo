//! Leased payment jobs; network runs after claim commit, fenced receipts prevent duplicate local effects.
use crate::domain::amounts::parse_amount;
use crate::domain::receipt_ref::identity;
use crate::ports::{Gateway, Host};
use crate::store::attempts;
use crate::store::jobs::{self, enqueue_tx};
use crate::store::order_ledger::release_stock;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Result, State, StatusCode, bad, conflict};

/// The provider answer fields the worker reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Answer {
    status: Value,
    id: Value,
    #[serde(deserialize_with = "lenient")]
    amount: AnswerAmount,
    not_created: Value,
    vault_revoked: Value,
    vault_reference: Value,
}

/// A provider money amount.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AnswerAmount {
    currency_code: Value,
    value: Value,
}

/// The job request fields the worker reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct JobInput {
    amount_minor: Value,
    expiry: Value,
    reference: Value,
}

/// The ledger job a provider operation runs for.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerJob<'a> {
    ledger_job_key: &'a str,
}

pub async fn payment_once(host: &dyn Host, gateway: &dyn Gateway, a: &State) -> Result<bool> {
    let (tenants, providers) = gateway.scope();
    if tenants.is_empty() && providers.is_empty() {
        return Ok(false);
    }
    let mut tx = a.db.begin().await?;
    // Take the attempt first everywhere. Reconciliation may finish a different capture job.
    let candidate = attempts::lock_due(&mut *tx, &tenants, &providers).await?;
    let row = if let Some(candidate) = candidate {
        jobs::lock_due(&mut tx, &candidate).await?
    } else {
        None
    };
    let Some(job) = row else {
        tx.commit().await?;
        return expire_one(a, &tenants, &providers).await;
    };
    let id = job.id;
    let op = job.operation;
    let mut input = job.request;
    merge(
        &mut input,
        LedgerJob {
            ledger_job_key: &id,
        },
    )?;
    let generation = job.attempts + 1;
    jobs::lease(&mut tx, &id, generation).await?;
    let p = attempts::get_claimed(&mut *tx, &job.attempt_id).await?;
    tx.commit().await?;
    let outcome = if ["capture", "authorize"].contains(&op.as_str())
        && ![
            "ready",
            "approved",
            "authorized",
            "captured",
            "partially_captured",
            "partially_refunded",
        ]
        .contains(&p.state.as_str())
    {
        Err(conflict("Payment cannot be captured in this state"))
    } else if op == "cancel" && p.provider == "paypal" {
        if p.provider_order.is_some() {
            gateway.dispatch(&p, "reconcile", &id, &input).await
        } else {
            Ok(json!({"notCreated":true}))
        }
    } else {
        gateway.dispatch(&p, &op, &id, &input).await
    };
    let mut tx = a.db.begin().await?;
    // Admission and receipt allocation lock attempt before jobs, including recovered capture jobs.
    let current = attempts::lock_claimed(&mut *tx, &p.id).await?;
    let fence = jobs::lock_fence(&mut tx, &id).await?;
    if fence.state != "running" || fence.attempts != generation {
        return Ok(false);
    }
    match outcome {
        Ok(v) => {
            let answer: Answer = view(&v);
            let job: JobInput = view(&input);
            if op == "refund" && p.provider == "paypal" && answer.status == "PENDING" {
                let remote = answer
                    .id
                    .as_str()
                    .filter(|s| s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
                    .ok_or(bad("Refund ID missing"))?;
                let amount = job
                    .amount_minor
                    .as_i64()
                    .ok_or(bad("Refund amount missing"))?;
                if answer.amount.currency_code != current.currency
                    || parse_amount(answer.amount.value.as_str().unwrap_or(""), &p.currency)?
                        != amount
                {
                    return Err(bad("Pending refund amount mismatch"));
                }
                jobs::record_pending_refund(&mut tx, (&p.tenant, &id, &p.id), remote, amount)
                    .await?;
                jobs::requeue(&mut tx, &id).await?;
                tx.commit().await?;
                return Ok(false);
            }
            // SAVEPOINT keeps a rejected/malformed remote receipt from partially mutating local state.
            jobs::begin_receipt(&mut tx).await?;
            let result =
                if op == "cancel" && current.provider == "paypal" && answer.status != "COMPLETED" {
                    let validation = if answer.not_created == true {
                        Ok(())
                    } else {
                        gateway.cancellable(&current, &v)
                    };
                    if let Err(e) = validation {
                        Err(e)
                    } else {
                        release_stock(
                            host,
                            &mut tx,
                            &current,
                            if job.expiry == true {
                                "expired"
                            } else {
                                "cancelled"
                            },
                            false,
                        )
                        .await
                    }
                } else if op == "vault_revoke" {
                    identity(&current, &v).and_then(|_| {
                        if answer.vault_revoked == true && answer.vault_reference == job.reference {
                            Ok(())
                        } else {
                            Err(bad("Provider did not confirm instrument revocation"))
                        }
                    })
                } else {
                    gateway.persist(&mut tx, &current, &op, &input, &v).await
                };
            if let Err(e) = result {
                if e.0 == StatusCode::ACCEPTED && generation < 8 {
                    jobs::discard_receipt(&mut tx).await?;
                    jobs::requeue(&mut tx, &id).await?;
                    tx.commit().await?;
                    return Ok(false);
                }
                jobs::discard_receipt(&mut tx).await?;
                jobs::uncertain(&mut tx, &id, &e.1).await?;
            } else {
                jobs::succeeded(&mut tx, &id, &v).await?;
            }
        }
        Err(e) => {
            let terminal = generation >= 8 || e.0 == StatusCode::CONFLICT;
            let state = if terminal {
                if e.0 == StatusCode::CONFLICT {
                    "failed"
                } else {
                    "uncertain"
                }
            } else {
                "queued"
            };
            jobs::failed(&mut tx, &id, state, &e.1).await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}
async fn expire_one(a: &State, tenants: &[String], providers: &[String]) -> Result<bool> {
    // An uncertain provider operation requires reconciliation, never automatic inventory release.
    let mut tx = a.db.begin().await?;
    let row = attempts::lock_expired(&mut *tx, tenants, providers).await?;
    let worked = row.is_some();
    if let Some(p) = row {
        enqueue_tx(
            &mut tx,
            &p.tenant,
            &p.id,
            "cancel",
            &format!("{}:expiry", p.id),
            &json!({"expiry":true}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(worked)
}

#[cfg(test)]
mod tests {
    use super::LedgerJob;
    use serde_json::json;
    use veyndo_core::document::merge;

    /// The typed job key writes exactly the operation input the indexed assignment wrote.
    #[test]
    fn the_typed_job_key_matches_the_indexed_write() {
        for base in [json!({"amountMinor":5}), json!(null)] {
            let mut indexed = base.clone();
            indexed["ledgerJobKey"] = json!("job-1");
            let mut typed = base.clone();
            merge(
                &mut typed,
                LedgerJob {
                    ledger_job_key: "job-1",
                },
            )
            .unwrap();
            assert_eq!(typed, indexed);
        }
    }
}
