//! Immutable capture receipts and exact cumulative settlement for native partial/multiple captures.
use crate::domain::attempt::Attempt;
use crate::domain::refunds;
use serde_json::{Value, json};

pub fn captured(p: &Attempt) -> i64 {
    p.context["capturedAmountMinor"]
        .as_i64()
        .unwrap_or_else(|| if p.capture.is_some() { p.amount } else { 0 })
}
pub fn closed(p: &Attempt) -> bool {
    p.context["captureClosed"]
        .as_bool()
        .unwrap_or_else(|| captured(p) == p.amount)
}
pub fn multi(p: &Attempt) -> bool {
    p.provider != "paypal"
        && p.context["intent"] == "authorize"
        && p.context["capabilities"]
            .as_array()
            .is_some_and(|cs| cs.iter().any(|c| c == "multi_capture"))
}
pub fn summary(p: &Attempt) -> Value {
    let captured = captured(p);
    let closed = closed(p);
    let mut receipts = p.context["captureReceipts"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if receipts.is_empty()
        && let Some(id) = &p.capture
    {
        receipts.insert(
            id.clone(),
            json!({"amountMinor":captured,"currency":p.currency}),
        );
    }
    let captures: Vec<Value> = receipts.iter().map(|(id, value)| json!({"id":id,"amountMinor":value["amountMinor"],"currency":p.currency,"refundedMinor":refunds::used(p,&p.context,id)})).collect();
    json!({"capturedMinor":captured,"refundedMinor":p.refunded,"captureClosed":closed,
        "remainingAuthorizationMinor":if closed {0}else{p.amount.saturating_sub(captured)},
        "multipleCapture":multi(p),"canCaptureMore":!closed && ["authorized","partially_captured","partially_refunded"].contains(&p.state.as_str()) && (p.context["authorizationId"].is_string() || p.state == "authorized")
            && (captured==0 || multi(p)),"captures":captures})
}
pub fn state(p: &Attempt, settled: i64, refunded: i64, closed: bool) -> &'static str {
    if refunded > 0 {
        if refunded == settled && closed {
            "refunded"
        } else {
            "partially_refunded"
        }
    } else if ["cancelled", "expired", "captured_late"].contains(&p.state.as_str()) {
        "captured_late"
    } else if settled == p.amount {
        "captured"
    } else {
        "partially_captured"
    }
}

#[cfg(test)]
mod tests {
    use super::{captured, closed, multi, state, summary};
    use crate::domain::attempt::{Attempt, sample};
    use serde_json::json;
    #[test]
    fn partial_funds_are_not_full_payment_or_a_released_reservation() {
        let p = Attempt {
            id: "a".into(),
            provider: "example".into(),
            context: json!({}),
            tenant: "t".into(),
            order: "o".into(),
            amount: 1000,
            currency: "EUR".into(),
            currency_scale: 2,
            state: "authorized".into(),
            provider_order: None,
            capture: None,
            refunded: 0,
            adapter_version: "1.3.0".into(),
            environment: "simulation".into(),
            bn_code: "".into(),
        };
        assert_eq!(state(&p, 200, 0, false), "partially_captured");
        assert_eq!(state(&p, 200, 200, false), "partially_refunded");
        assert_eq!(state(&p, 200, 200, true), "refunded");
        assert_eq!(state(&p, 1000, 0, true), "captured");
    }
    #[test]
    fn captured_and_closed_follow_the_recorded_receipts_or_the_single_capture() {
        let mut p = sample();
        assert_eq!((captured(&p), closed(&p)), (0, false));
        p.capture = Some("cap-1".into());
        assert_eq!((captured(&p), closed(&p)), (1000, true));
        p.context = json!({"capturedAmountMinor":400});
        assert_eq!((captured(&p), closed(&p)), (400, false));
        p.context = json!({"capturedAmountMinor":400,"captureClosed":true});
        assert!(closed(&p));
    }

    #[test]
    fn multiple_capture_needs_an_authorization_and_the_declared_capability() {
        let mut p = sample();
        p.context = json!({"intent":"authorize","capabilities":["multi_capture"]});
        assert!(multi(&p));
        p.provider = "paypal".into();
        assert!(!multi(&p));
        p.provider = "example".into();
        p.context = json!({"intent":"capture","capabilities":["multi_capture"]});
        assert!(!multi(&p));
        p.context = json!({"intent":"authorize","capabilities":["refund"]});
        assert!(!multi(&p));
    }

    #[test]
    fn the_summary_reports_captures_refunds_and_the_remaining_authorization() {
        let mut p = sample();
        p.context = json!({"capturedAmountMinor":400,"captureReceipts":{"cap-1":{"amountMinor":400}},
            "providerRefunds":{"r1":{"captureId":"cap-1","amountMinor":150}}});
        p.refunded = 150;
        let s = summary(&p);
        assert_eq!(s["capturedMinor"], 400);
        assert_eq!(s["refundedMinor"], 150);
        assert_eq!(s["captureClosed"], false);
        assert_eq!(s["remainingAuthorizationMinor"], 600);
        assert_eq!(s["multipleCapture"], false);
        assert_eq!(
            s["captures"],
            json!([{"id":"cap-1","amountMinor":400,"currency":"EUR","refundedMinor":150}])
        );
        let mut single = sample();
        single.capture = Some("cap-9".into());
        let s = summary(&single);
        assert_eq!(s["remainingAuthorizationMinor"], 0);
        assert_eq!(s["captures"][0]["id"], "cap-9");
        assert_eq!(s["captures"][0]["amountMinor"], 1000);
    }
}
