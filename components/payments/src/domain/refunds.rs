//! Refund budgets: which capture a refund draws from, what is already used and the exact admissible total.
use crate::domain::attempt::Attempt;
use crate::domain::receipt_ref::reference;
use crate::domain::settlements;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::view;
use veyndo_core::{Result, bad, conflict};
use veyndo_pricing::verified_kernel;

/// One refund: the capture it draws from and its amount.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RefundEntry {
    capture_id: Value,
    amount_minor: Value,
}

/// The JSON fields `capture` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CaptureInput {
    capture_receipts: Value,
}

pub fn capture(p: &Attempt, context: &Value, refund: &Value) -> Result<(String, i64)> {
    let input: CaptureInput = view(context);
    let captures = input.capture_receipts.as_object();
    let requested: RefundEntry = view(refund);
    let id = requested
        .capture_id
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
        .and_then(|c| view::<RefundEntry>(c).amount_minor.as_i64())
        .or_else(|| (p.capture.as_deref() == Some(id)).then(|| settlements::captured(p)))
        .filter(|n| *n > 0)
        .ok_or(bad("Refund capture does not belong to this payment"))?;
    Ok((id.into(), amount))
}
/// The JSON fields `used` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct UsedInput {
    provider_refunds: Value,
}

pub fn used(p: &Attempt, context: &Value, capture: &str) -> i64 {
    let input: UsedInput = view(context);
    let Some(rows) = input.provider_refunds.as_object() else {
        return p.refunded;
    };
    let known: i64 = rows
        .values()
        .map(view::<RefundEntry>)
        .filter(|r| r.capture_id.as_str().or(p.capture.as_deref()) == Some(capture))
        .filter_map(|r| r.amount_minor.as_i64())
        .fold(0_i64, i64::saturating_add);
    let total: i64 = rows
        .values()
        .filter_map(|r| view::<RefundEntry>(r).amount_minor.as_i64())
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
    use super::{capture, checked_total, used};
    use crate::domain::attempt::sample;
    use serde_json::json;
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
    #[test]
    fn a_refund_draws_from_a_capture_of_this_payment() {
        let mut p = sample();
        let receipts =
            json!({"captureReceipts":{"cap-1":{"amountMinor":400},"cap-2":{"amountMinor":600}}});
        assert_eq!(
            capture(&p, &receipts, &json!({"captureId":"cap-2"})).ok(),
            Some(("cap-2".into(), 600))
        );
        assert!(capture(&p, &receipts, &json!({})).is_err());
        assert!(capture(&p, &receipts, &json!({"captureId":"cap-3"})).is_err());
        assert!(capture(&p, &receipts, &json!({"captureId":"cap 1"})).is_err());
        let single = json!({"captureReceipts":{"cap-1":{"amountMinor":400}}});
        assert_eq!(
            capture(&p, &single, &json!({})).ok(),
            Some(("cap-1".into(), 400))
        );
        assert!(capture(&p, &json!({}), &json!({})).is_err());
        p.capture = Some("cap-9".into());
        assert_eq!(
            capture(&p, &json!({}), &json!({})).ok(),
            Some(("cap-9".into(), 1000))
        );
        p.context = json!({"capturedAmountMinor":0});
        assert!(capture(&p, &json!({}), &json!({})).is_err());
    }

    #[test]
    fn used_budget_counts_known_refunds_and_the_unattributed_rest_on_the_first_capture() {
        let mut p = sample();
        p.refunded = 300;
        assert_eq!(used(&p, &json!({}), "cap-1"), 300);
        p.capture = Some("cap-1".into());
        let context = json!({"providerRefunds":{
            "r1":{"captureId":"cap-1","amountMinor":100},
            "r2":{"captureId":"cap-2","amountMinor":50},
            "r3":{"amountMinor":25}}});
        assert_eq!(used(&p, &context, "cap-1"), 100 + 25 + (300 - 175));
        assert_eq!(used(&p, &context, "cap-2"), 50);
        p.refunded = 100;
        assert_eq!(used(&p, &context, "cap-1"), 125);
    }
}
