//! Property tests for applying promotions to a priced quote: every discount stays within its base, no total or line goes negative, exclusive promotions stop the chain.
use super::apply;
use crate::types::promotion::Promotion;
use proptest::prelude::*;
use serde_json::{Value, json};
use veyndo_pricing::pricing::math_round;

/// A promotion as generated: kind, amount and whether it is exclusive.
type Spec = (&'static str, f64, bool);

fn promotion((kind, amount, exclusive): Spec) -> (String, Promotion) {
    let p = serde_json::from_value(json!({"name":{"en":kind},"active":true,"code":null,"kind":kind,"amount":amount,
        "rule":{"type":"alwaysValid"},"exclusive":exclusive,"priority":1,"maxUses":null,"start":null,"end":null}))
    .unwrap();
    (format!("{kind}-{amount}"), p)
}

fn spec() -> impl Strategy<Value = Spec> {
    (
        prop::sample::select(vec!["percentage", "absolute", "free_shipping"]),
        0_u32..=100_000,
        prop::bool::weighted(0.2),
    )
        .prop_map(|(kind, amount, exclusive)| {
            let amount = if kind == "percentage" {
                f64::from(amount % 10_001) / 100.
            } else {
                f64::from(amount) / 100.
            };
            (kind, amount, exclusive)
        })
}

/// A priced quote of lines given as gross cents and tax rate.
fn quote(net: bool, lines: &[(u32, f64)]) -> Value {
    let items: Vec<Value> = lines
        .iter()
        .map(|(cents, rate)| {
            let total = f64::from(*cents) / 100.;
            // Taxes as the cart quote calculates them: contained in a gross line, added to a net one.
            let tax = if net {
                math_round(total * rate / 100., 2)
            } else {
                math_round(total - total / (1. + rate / 100.), 2)
            };
            json!({"id":"l","quantity":1,"price":{"unitPrice":total,"totalPrice":total,
                "calculatedTaxes":[{"tax":tax,"taxRate":rate,"price":total}]}})
        })
        .collect();
    json!({"price":{"taxStatus":if net {"net"} else {"gross"},"currencyScale":2,"currency":"EUR"},
        "currencyContext":null,"lineItems":items})
}

fn amount(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The basket discount is never negative and never above the original total; the
    /// promoted totals, every line and every single discount stay non-negative; no line rises.
    #[test]
    fn promotions_stay_within_their_base(
        net in any::<bool>(),
        lines in prop::collection::vec((1_u32..500_000, prop::sample::select(vec![0., 7., 19.])), 1..6),
        specs in prop::collection::vec(spec(), 0..5),
    ) {
        let original = quote(net, &lines);
        let mut q = original.clone();
        apply(&mut q, specs.iter().copied().map(promotion).collect(), &[]).unwrap();
        let before: f64 = original["lineItems"].as_array().unwrap().iter().map(|l| amount(&l["price"]["totalPrice"])).sum();
        let tax: f64 = original["lineItems"].as_array().unwrap().iter().map(|l| amount(&l["price"]["calculatedTaxes"][0]["tax"])).sum();
        let base = math_round(if net { before + tax } else { before }, 2);
        let discount = amount(&q["discountTotal"]);
        prop_assert!(discount >= 0. && discount <= base + 1e-9, "discount {discount} of {base}");
        prop_assert!(amount(&q["price"]["totalPrice"]) >= 0.);
        prop_assert!(amount(&q["price"]["positionPrice"]) >= 0.);
        for d in q["discounts"].as_array().unwrap() {
            prop_assert!(amount(&d["amount"]) >= 0.);
        }
        for (after, before) in q["lineItems"].as_array().unwrap().iter().zip(original["lineItems"].as_array().unwrap()) {
            let (after, before) = (amount(&after["price"]["totalPrice"]), amount(&before["price"]["totalPrice"]));
            prop_assert!(after >= 0. && after <= before + 1e-9);
        }
    }

    /// No promotion after the first exclusive one is applied, and free shipping alone changes no amount.
    #[test]
    fn exclusive_promotions_end_the_chain(
        lines in prop::collection::vec((1_u32..500_000, prop::sample::select(vec![0., 7., 19.])), 1..4),
        specs in prop::collection::vec(spec(), 1..6),
    ) {
        let mut q = quote(false, &lines);
        apply(&mut q, specs.iter().copied().map(promotion).collect(), &[]).unwrap();
        let applied = q["discounts"].as_array().unwrap().len();
        if let Some(first) = specs.iter().position(|(_, _, exclusive)| *exclusive) {
            prop_assert!(applied <= first + 1);
        }
        let mut before = quote(false, &lines);
        apply(&mut before, vec![], &[]).unwrap();
        let mut shipping = quote(false, &lines);
        apply(&mut shipping, vec![promotion(("free_shipping", 0., false))], &[]).unwrap();
        prop_assert_eq!(amount(&shipping["discountTotal"]), 0.);
        prop_assert_eq!(&shipping["price"]["totalPrice"], &before["price"]["totalPrice"]);
        prop_assert_eq!(shipping["promotionFreeShipping"].as_bool(), Some(true));
    }
}
