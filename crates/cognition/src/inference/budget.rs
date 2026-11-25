//! Durable conservative model allocations use existing agent keys and the live shop stop switch.
//! Counters reserve full envelopes before egress and never refund uncertain calls.
use super::Admission;
use crate::store::budget as stored;
use crate::{scoped_pool::ScopedPool, tenant_scope};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use veyndo_core::document::{lenient, view};

#[derive(Clone, Debug)]
pub struct Authority {
    pub tenant: String,
    pub key_id: Option<String>,
    pub require_budget: bool,
}
tokio::task_local! { static CURRENT: Authority; }
pub fn is_budgeted() -> bool {
    CURRENT
        .try_with(|a| a.require_budget || a.key_id.is_some())
        .unwrap_or(false)
}
pub fn scoped<F: Future>(authority: Authority, future: F) -> impl Future<Output = F::Output> {
    // Construct the small scope synchronously. An async wrapper would retain F
    // inline and multiply large HTTP/POS polling frames in instrumented builds.
    CURRENT.scope(authority, Box::pin(future))
}
/// A key selects its approved model by default; client/provider routing cannot widen that choice.
pub async fn choice(
    db: Option<&ScopedPool>,
    requested: Option<&super::Choice>,
) -> Result<Option<super::Choice>, Admission> {
    let Some(authority) = CURRENT
        .try_with(Clone::clone)
        .ok()
        .filter(|a| a.key_id.is_some())
    else {
        return Ok(requested.cloned());
    };
    let db = db.ok_or("Agent model admission requires durable storage")?;
    let value: Value = tenant_scope::scoped(
        tenant_scope::Scope::System,
        stored::approved(db, &authority.tenant, authority.key_id.as_deref()),
    )
    .await
    .map_err(|_| "Agent model admission unavailable")?
    .ok_or(Admission::Denied("Agent key expired or revoked"))?;
    let budget: Budget = serde_json::from_value(value).map_err(|_| "Invalid agent budget")?;
    budget.validate()?;
    if let Some(c) = requested
        && !matches!(c.provider, super::Provider::Platform)
        && (serde_json::to_value(&c.provider).unwrap().as_str() != Some(budget.provider.as_str())
            || c.model.as_ref().is_some_and(|m| m != &budget.model))
    {
        return Err(Admission::Denied(
            "Agent provider or model differs from its approved budget",
        ));
    }
    Ok(Some(super::Choice {
        provider: serde_json::from_value(json!(budget.provider))
            .map_err(|_| "Invalid agent provider")?,
        model: Some(budget.model),
    }))
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Budget {
    pub provider: String,
    pub model: String,
    pub tokens: u64,
    pub cost_micros: u64,
    pub price_per_million_micros: u64,
}
impl Budget {
    pub fn validate(&self) -> Result<(), String> {
        if !["openai", "anthropic", "ollama"].contains(&self.provider.as_str())
            || self.model.is_empty()
            || self.model.len() > 128
            || !self.model.is_ascii()
            || self.model.chars().any(char::is_control)
        {
            return Err("An exact agent provider and model are required".into());
        }
        if [self.tokens, self.cost_micros, self.price_per_million_micros]
            .into_iter()
            .all(|n| (1..=1_000_000_000_000).contains(&n))
        {
            Ok(())
        } else {
            Err(
                "Explicit positive bounded agent token, USD micro-unit and tariff limits required"
                    .into(),
            )
        }
    }
}

/// The charge is an allocation at the operator-selected ceiling tariff, not a provider invoice.
pub fn allocation(request: &Value, images: bool, tariff: u64) -> Result<(u64, u64), String> {
    if images {
        return Err("Budgeted agents currently require text-only model input".into());
    }
    let output = request["max_output_tokens"]
        .as_u64()
        .or_else(|| request["max_completion_tokens"].as_u64())
        .or_else(|| request["max_tokens"].as_u64())
        .or_else(|| request["options"]["num_predict"].as_u64())
        .filter(|n| *n > 0 && *n <= 8192)
        .ok_or("Model request has no bounded output allowance")?;
    // UTF-8 request bytes deliberately overcount ordinary text tokens. Reserve an
    // additional framing allowance; provider-specific metering is not an invoice guarantee.
    let tokens = u64::try_from(request.to_string().len())
        .map_err(|_| "Agent envelope overflow")?
        .checked_add(output)
        .and_then(|n| n.checked_add(4096))
        .ok_or("Agent envelope overflow")?;
    let cost = tokens
        .checked_mul(tariff)
        .and_then(|n| n.checked_add(999_999))
        .map(|n| n / 1_000_000)
        .ok_or("Agent tariff overflow")?;
    Ok((tokens, cost))
}

/// The JSON fields `reserve` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReserveInput {
    #[serde(deserialize_with = "lenient")]
    model: Option<String>,
}

pub async fn reserve(
    db: Option<&ScopedPool>,
    request: &Value,
    images: bool,
    provider: &str,
) -> Result<(), Admission> {
    let input: ReserveInput = view(request);
    let Some(authority) = CURRENT.try_with(Clone::clone).ok() else {
        return Ok(());
    };
    if authority.tenant.is_empty() && !authority.require_budget {
        return Ok(());
    }
    let db = db.ok_or("Agent model admission requires durable storage")?;
    tenant_scope::scoped(tenant_scope::Scope::System, async {
        let mut tx = db
            .begin()
            .await
            .map_err(|_| "Agent admission unavailable")?;
        // Same order as stop/key lifecycle: live tenant first, then key. No lock
        // survives the provider call. A stop affects subsequent calls, not past egress.
        let shop = stored::lock_shop(&mut tx, &authority.tenant)
            .await
            .map_err(|_| "Agent shop unavailable")?;
        let live = shop.id;
        let stopped = shop.agents_stopped;
        if stopped {
            return Err(Admission::Denied(
                "Shop agents are stopped by an administrator",
            ));
        }
        let Some(id) = &authority.key_id else {
            if authority.require_budget {
                return Err(Admission::Denied(
                    "Use a dedicated agent key with explicit budgets",
                ));
            }
            return Ok(());
        };
        let row = stored::lock_key(&mut tx, &live, id)
            .await
            .map_err(|_| "Agent admission unavailable")?
            .ok_or(Admission::Denied(
                "Agent key expired, revoked or membership withdrawn",
            ))?;
        let budget: Budget = serde_json::from_value(row.agent_budget.unwrap_or_default())
            .map_err(|_| "Agent budget is missing or invalid")?;
        budget.validate()?;
        if budget.provider != provider || input.model.as_deref() != Some(budget.model.as_str()) {
            return Err(Admission::Denied(
                "Agent provider or model differs from its approved budget",
            ));
        }
        let (tokens, cost) = allocation(request, images, budget.price_per_million_micros)?;
        let used_tokens =
            u64::try_from(row.agent_tokens_allocated).map_err(|_| "Invalid agent usage")?;
        let used_cost =
            u64::try_from(row.agent_cost_allocated).map_err(|_| "Invalid agent usage")?;
        if !veyndo_kernel::verified_kernel::b2b_budget_admissible(
            budget.tokens,
            used_tokens,
            tokens,
        ) || !veyndo_kernel::verified_kernel::b2b_budget_admissible(
            budget.cost_micros,
            used_cost,
            cost,
        ) {
            return Err(Admission::Exhausted);
        }
        stored::allocate(&mut tx, &live, id, tokens as i64, cost as i64)
            .await
            .map_err(|_| "Agent usage could not be reserved")?;
        stored::audit(
            &mut tx,
            &live,
            &json!({"agentId":id,"tokens":tokens,"costMicros":cost,"currency":"USD"}),
        )
        .await
        .map_err(|_| "Agent audit could not be saved")?;
        tx.commit()
            .await
            .map_err(|_| Admission::Unavailable("Agent admission could not commit"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn authority_scope_bounds_a_large_child_and_never_leaks_context() {
        let payload = [7u8; 64 * 1024];
        let future = scoped(
            Authority {
                tenant: "synthetic".into(),
                key_id: Some("fixture".into()),
                require_budget: true,
            },
            async move {
                assert!(is_budgeted());
                std::hint::black_box(payload).len()
            },
        );
        assert!(
            std::mem::size_of_val(&future) < 1024,
            "Scope must box its child before constructing its own future"
        );
        assert!(!is_budgeted());
        assert_eq!(future.await, 64 * 1024);
        assert!(!is_budgeted());
    }
    #[test]
    fn explicit_bounded_envelopes_reject_missing_limits_images_and_overflow() {
        let budget = Budget {
            provider: "openai".into(),
            model: "synthetic".into(),
            tokens: 100_000,
            cost_micros: 1000,
            price_per_million_micros: 100,
        };
        assert!(budget.validate().is_ok());
        assert!(
            Budget {
                tokens: 0,
                ..budget
            }
            .validate()
            .is_err()
        );
        let r = json!({"max_output_tokens":8192,"input":"hello"});
        let (tokens, cost) = allocation(&r, false, 1_000_000).unwrap();
        assert!(tokens > 8192);
        assert_eq!(tokens, cost);
        assert!(allocation(&r, true, 10).is_err());
        assert!(allocation(&json!({"input":"hello"}), false, 10).is_err());
        assert!(allocation(&r, false, u64::MAX).is_err());
    }
}
