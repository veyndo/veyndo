//! Refund budgets: which capture a refund draws from, what is already used and the exact admissible total.
use crate::domain::attempt::Attempt;
use crate::domain::receipt_ref::reference;
use crate::domain::settlements;
use serde_json::Value;
use veyndo_core::{Result, bad, conflict};
use veyndo_pricing::verified_kernel;

pub fn capture(p: &Attempt, context: &Value, refund: &Value) -> Result<(String, i64)> {
    let captures = context["captureReceipts"].as_object();
    let id = refund["captureId"]
        .as_str()
        .or_else(|| match captures {
            Some(cs) if cs.len() == 1 => cs.keys().next().map(String::as_str),
            None => p.capture.as_deref(),
            _ => None,
        })
        .ok_or(bad("Select the capture to refund"))?;
    reference(&Value::String(id.into()))?;
    let amount = captures
        .and_then(|cs| cs.get(id))
        .and_then(|c| c["amountMinor"].as_i64())
        .or_else(|| (p.capture.as_deref() == Some(id)).then(|| settlements::captured(p)))
        .filter(|n| *n > 0)
        .ok_or(bad("Refund capture does not belong to this payment"))?;
    Ok((id.into(), amount))
}
pub fn used(p: &Attempt, context: &Value, capture: &str) -> i64 {
    let Some(rows) = context["providerRefunds"].as_object() else {
        return p.refunded;
    };
    let known: i64 = rows
        .values()
        .filter(|r| r["captureId"].as_str().or(p.capture.as_deref()) == Some(capture))
        .filter_map(|r| r["amountMinor"].as_i64())
        .fold(0_i64, i64::saturating_add);
    let total: i64 = rows
        .values()
        .filter_map(|r| r["amountMinor"].as_i64())
        .fold(0_i64, i64::saturating_add);
    known
        + if p.capture.as_deref() == Some(capture) {
            p.refunded.saturating_sub(total).max(0)
        } else {
            0
        }
}
pub fn checked_total(captured: i64, refunded: i64, delta: i64) -> Result<i64> {
    let valid = u64::try_from(captured)
        .ok()
        .zip(u64::try_from(refunded).ok())
        .zip(u64::try_from(delta).ok())
        .is_some_and(|((captured, refunded), delta)| {
            (delta == 0 && refunded <= captured)
                || verified_kernel::refund_admissible(captured, refunded, delta)
        });
    if !valid {
        return Err(conflict("Provider refunds exceed payment"));
    }
    refunded
        .checked_add(delta)
        .ok_or(conflict("Provider refund overflow"))
}

#[cfg(test)]
mod tests {
    use super::checked_total;
    #[test]
    fn confirmed_delta_reuses_exact_refund_policy_and_allows_zero_for_replay() {
        assert_eq!(checked_total(1250, 1249, 1).unwrap(), 1250);
        assert_eq!(checked_total(1250, 1250, 0).unwrap(), 1250);
        for (captured, refunded, delta) in [
            (1250, 1249, 2),
            (1250, 1251, 0),
            (-1, 0, 1),
            (1250, 0, -1),
            (i64::MAX, i64::MAX, 1),
        ] {
            assert!(checked_total(captured, refunded, delta).is_err());
        }
    }
}
