//! The exact calculator against the previous floating-point port on the differential suite's input population.
//!
//! `scripts/differential.py` compares the calculator with the original PHP classes in CI. This test
//! compares it with the floating-point port that passed that comparison, on generated inputs of the
//! same kind, field by field and bit for bit. `PRICE_CASES=<file.json>` adds a recorded population.
#[path = "support/float_reference.rs"]
mod float_reference;

use proptest::prelude::*;
use serde_json::{Value, json};
use veyndo_pricing::pricing::{PriceInput, calculate};

/// The first difference between two outputs; a zero matches a negative zero (see below).
fn difference(path: &str, float: &Value, exact: &Value) -> Option<String> {
    match (float, exact) {
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path}: {float} != {exact}"));
            }
            a.iter().find_map(|(k, v)| {
                difference(&format!("{path}.{k}"), v, b.get(k).unwrap_or(&Value::Null))
            })
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .find_map(|(i, (x, y))| difference(&format!("{path}[{i}]"), x, y)),
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            // The float port writes -0.0 when a negative input rounds to zero; the exact
            // calculator has one zero. Numerically equal, and production never prices negatives.
            (a.to_bits() != b.to_bits() && !(a == 0. && b == 0.))
                .then(|| format!("{path}: {a:?} != {b:?}"))
        }
        _ => (float != exact).then(|| format!("{path}: {float} != {exact}")),
    }
}

/// Both calculators on one wire input; `Err` names the first differing field.
fn compare(input: &Value) -> Result<(), String> {
    let float: float_reference::PriceInput = serde_json::from_value(input.clone()).unwrap();
    let exact: PriceInput = serde_json::from_value(input.clone()).unwrap();
    let float = serde_json::to_value(float_reference::calculate(&float)).unwrap();
    let exact = match calculate(&exact) {
        Ok(price) => serde_json::to_value(price).unwrap(),
        Err(e) => return Err(format!("{input}: refused: {e}")),
    };
    match difference("", &float, &exact) {
        Some(d) => Err(format!("{input}: {d}")),
        None => Ok(()),
    }
}

fn rounded(value: f64, digits: i32) -> f64 {
    let factor = 10_f64.powi(digits);
    (value * factor).round() / factor
}

#[test]
fn recorded_population_matches_when_given() {
    let Ok(path) = std::env::var("PRICE_CASES") else {
        return;
    };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let failures: Vec<String> = cases.iter().filter_map(|c| compare(c).err()).collect();
    assert!(
        failures.is_empty(),
        "{} of {} differ, first: {:?}",
        failures.len(),
        cases.len(),
        failures.first()
    );
}

#[test]
fn the_fixed_differential_grid_matches() {
    for price in [
        0., 0.005, 0.015, 1.005, 1.004999, 1.005001, 19.995, 100.125, -1.005, -19.995, 999.9999,
    ] {
        for quantity in [1, 3, 19] {
            for gross in [true, false] {
                for calculated in [true, false] {
                    for interval in [0.01, 0.05] {
                        let input = json!({"price":price,"quantity":quantity,"tax_rate":19,"gross":gross,
                            "calculated":calculated,"decimals":2,"interval":interval,"round_for_net":false});
                        compare(&input).unwrap();
                    }
                }
            }
        }
    }
    for rules in [
        json!([]),
        json!([{"tax_rate":19,"percentage":70},{"tax_rate":7,"percentage":30}]),
        json!([{"tax_rate":19,"percentage":30},{"tax_rate":19,"percentage":70}]),
        json!([{"tax_rate":0,"percentage":100}]),
    ] {
        for price in [0., 1.005, 19.995, -19.995, 200.123] {
            for gross in [true, false] {
                for calculated in [true, false] {
                    let input = json!({"price":price,"quantity":3,"tax_rate":19,"tax_rules":rules,"gross":gross,
                        "calculated":calculated,"list_price":price*1.3,"regulation_price":price*0.9,
                        "reference":{"purchase_unit":0.35,"reference_unit":1,"unit_name":"l"}});
                    compare(&input).unwrap();
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// The random part of the differential population: one rate, every rounding mode.
    #[test]
    fn single_rate_prices_match(
        price in -1000.0_f64..1000.0, digits in 0_i32..=5, quantity in 0_u32..=1000,
        rate in prop::sample::select(vec![0., 5., 7., 7.7, 19., 20., 21., 25.5]),
        gross in any::<bool>(), calculated in any::<bool>(), decimals in prop::sample::select(vec![0, 1, 2, 3, 4, 6]),
        interval in prop::sample::select(vec![0.01, 0.05, 0.1, 0.5, 1.]), round_for_net in any::<bool>(),
    ) {
        let input = json!({"price":rounded(price, digits),"quantity":quantity,"tax_rate":rate,"gross":gross,
            "calculated":calculated,"decimals":decimals,"interval":interval,"round_for_net":round_for_net});
        prop_assert_eq!(compare(&input), Ok(()));
    }

    /// Two-rate splits with list, regulation and reference prices, as the differential's second population.
    #[test]
    fn split_rates_with_ancillary_prices_match(
        price in -100_000.0_f64..100_000.0, digits in 0_i32..=4, quantity in 0_u32..=100,
        share in prop::sample::select(vec![0., 12.5, 33.33, 50., 66.67, 70., 100.]),
        gross in any::<bool>(), calculated in any::<bool>(), decimals in 2_i32..=3,
        interval in prop::sample::select(vec![0.01, 0.05]), round_for_net in any::<bool>(),
        list in prop::sample::select(vec![0., 1.5, -5., 1. / 1.19]), regulation in prop::sample::select(vec![0., 0.8]),
        purchase in prop::sample::select(vec![0., -1., 0.35, 2., 120., 0.333]),
        reference in prop::sample::select(vec![0., -1., 1., 100., 0.75]),
    ) {
        let price = rounded(price, digits);
        let list = if list == -5. { -5. } else { price * list };
        let input = json!({"price":price,"quantity":quantity,"tax_rate":19,
            "tax_rules":[{"tax_rate":19,"percentage":share},{"tax_rate":7,"percentage":100. - share}],
            "gross":gross,"calculated":calculated,"decimals":decimals,"interval":interval,"round_for_net":round_for_net,
            "list_price":list,"regulation_price":price * regulation,
            "reference":{"purchase_unit":purchase,"reference_unit":reference,"unit_name":"unit"}});
        prop_assert_eq!(compare(&input), Ok(()));
    }
}
