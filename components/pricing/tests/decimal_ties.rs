//! Known differences at decimal ties: where the exact calculator departs from the earlier binary64 port.
//!
//! Until 26 November the calculator reproduced Shopware's IEEE-754 binary64 steps, so a decimal
//! tie was decided by the binary error of the product and the original's fourteen-digit
//! precision step. The exact calculator rounds the exact value once, half away from zero. A
//! survey of 40,344 generated inputs (the former differential population) found 481 outputs that
//! changed, each by one minor unit: 280 on inputs with at most twelve significant digits, every
//! one an exact decimal tie (checked against an independent rational recomputation), and 201
//! on inputs carrying binary noise such as `price * 1.3` (`60969.149999999994`), which the
//! fourteen-digit step had snapped onto a tie. This table records representative cases with the
//! binary64 port's result and the exact one; every other surveyed field was unchanged.
use serde_json::{Value, json};
use veyndo_pricing::pricing::{PriceInput, calculate};

/// (input, field, binary64 result, exact result).
fn cases() -> Vec<(Value, &'static str, f64, f64)> {
    let split =
        |low: f64| json!([{"tax_rate":19,"percentage":low},{"tax_rate":7,"percentage":100. - low}]);
    vec![
        (
            json!({"price":274.0,"quantity":417,"tax_rate":20,"gross":true,"calculated":false,"decimals":0,"interval":0.05,"round_for_net":true}),
            "/tax",
            22865.,
            22866.,
        ),
        (
            json!({"price":191.0,"quantity":265,"tax_rate":7,"gross":false,"decimals":1,"interval":0.1}),
            "/tax",
            3543.,
            3543.1,
        ),
        (
            json!({"price":651.0,"quantity":73,"tax_rate":5,"gross":false,"decimals":1,"interval":0.5,"round_for_net":true}),
            "/calculated_taxes/0/tax",
            2376.1,
            2376.2,
        ),
        (
            json!({"price":-95.0,"quantity":87,"tax_rate":21,"gross":false,"decimals":1,"interval":1.0,"round_for_net":true}),
            "/tax",
            -1735.6,
            -1735.7,
        ),
        (
            json!({"price":682.43,"quantity":205,"tax_rate":7,"gross":false,"calculated":false,"decimals":3,"interval":0.5,"round_for_net":true}),
            "/tax",
            9792.87,
            9792.871,
        ),
        (
            json!({"price":-182.663,"quantity":155,"tax_rate":5,"gross":false,"decimals":2,"interval":0.5}),
            "/tax",
            -1415.61,
            -1415.62,
        ),
        (
            json!({"price":-91.847,"quantity":741,"tax_rate":20,"gross":true,"decimals":2,"interval":0.01}),
            "/tax",
            -11343.47,
            -11343.48,
        ),
        (
            json!({"price":100.205,"quantity":6,"tax_rate":7,"gross":true,"calculated":false,"decimals":4,"interval":0.01}),
            "/unit_price",
            107.2193,
            107.2194,
        ),
        (
            json!({"price":100.205,"quantity":6,"tax_rate":7,"gross":true,"calculated":false,"decimals":4,"interval":0.01}),
            "/total_price",
            643.3158,
            643.3164,
        ),
        (
            json!({"price":614.0,"quantity":78,"tax_rate":19,"tax_rules":split(70.),"gross":false,"calculated":false,"decimals":3,"interval":0.05,
            "reference":{"purchase_unit":120.0,"reference_unit":0.75,"unit_name":"unit"}}),
            "/reference_price/price",
            3.837,
            3.838,
        ),
        (
            json!({"price":93531.0,"quantity":22,"tax_rate":19,"tax_rules":split(50.),"gross":true,"calculated":false,"decimals":2,"interval":0.01,
            "round_for_net":true,"list_price":140296.5}),
            "/list_price/price",
            158535.04,
            158535.05,
        ),
        (
            json!({"price":61779.0,"quantity":19,"tax_rate":19,"tax_rules":split(70.),"gross":true,"calculated":false,"decimals":2,"interval":0.05,
            "round_for_net":true}),
            "/calculated_taxes/1/price",
            406369.81,
            406369.82,
        ),
    ]
}

#[test]
fn ties_follow_the_decimal_not_the_binary_error() {
    for (input, field, binary, exact) in cases() {
        let parsed: PriceInput = serde_json::from_value(input.clone()).unwrap();
        let price = serde_json::to_value(calculate(&parsed).unwrap()).unwrap();
        let value = price.pointer(field).and_then(Value::as_f64).unwrap();
        assert_eq!(value, exact, "{field} of {input}");
        assert_ne!(value, binary, "{field} of {input}");
    }
}

#[test]
fn values_away_from_ties_are_unchanged() {
    // The fixed differential grid's tie-free corner: the same amounts as the binary64 port.
    for (price, quantity, unit, total, tax) in [
        (19.995, 3, 20.0, 60.0, 9.58),
        (1.004999, 19, 1.0, 19.0, 3.03),
        (999.9999, 1, 1000.0, 1000.0, 159.66),
        (100.125, 3, 100.13, 300.39, 47.96),
    ] {
        let input: PriceInput =
            serde_json::from_value(json!({"price":price,"quantity":quantity,"tax_rate":19}))
                .unwrap();
        let p = serde_json::to_value(calculate(&input).unwrap()).unwrap();
        assert_eq!(
            (
                p["unit_price"].as_f64(),
                p["total_price"].as_f64(),
                p["tax"].as_f64()
            ),
            (Some(unit), Some(total), Some(tax)),
            "{price} × {quantity}"
        );
    }
}
