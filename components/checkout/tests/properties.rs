//! Property tests for the cart quote: totals are the sum of line totals plus tax in whole minor units, and the same cart always gives the same quote.
use proptest::prelude::*;
use serde_json::{Value, json};
use veyndo_checkout::domain::cart_quote::{normalized_quantity, quote};
use veyndo_pricing::catalog::Product;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::StoredCart;

fn settings() -> Settings {
    serde_json::from_str(include_str!("../../../fixtures/demo-settings.json")).unwrap()
}

fn product(id: usize, cents: u32, rate: f64, min: u32, steps: u32) -> Product {
    serde_json::from_value(json!({
        "extra":{},"id":format!("p{id}"),"name":"Lamp","category":"","description":"",
        "price":f64::from(cents) / 100.,"tax_rate":rate,"stock":5,"revision":1,"list_price":null,
        "regulation_price":null,"reference_price":null,"advanced_prices":[],"min_purchase":min,
        "purchase_steps":steps,"max_purchase":null,"parent_id":null,"options":{},"media":[],
        "properties":{},"delivery_days":1
    }))
    .unwrap()
}

fn cart(items: Value, group: &str) -> StoredCart {
    StoredCart {
        id: "c1".into(),
        tenant: "demo".into(),
        token: "t".into(),
        data: serde_json::from_value(json!({"items":items,"group":group,"email":null,"company":null,"session":"s","buyer":null,"order":null})).unwrap(),
        revision: 1,
        status: "open".into(),
    }
}

/// A quote amount in whole cents (the demo shop's currency has two decimals).
fn cents(value: &Value) -> i64 {
    (value.as_f64().unwrap() * 100.).round() as i64
}

/// Lines of a generated cart: price in cents, tax rate and quantity.
fn lines() -> impl Strategy<Value = Vec<(u32, f64, u32)>> {
    prop::collection::vec(
        (
            1_u32..2_000_000,
            prop::sample::select(vec![0., 7., 19.]),
            1_u32..50,
        ),
        1..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Order total: the payable total is the sum of line totals (gross quotes, which contain
    /// the tax) or the sum of line totals plus the tax (net quotes); net plus tax is the
    /// gross; the tax is never negative and never above the gross; repeated quoting is identical.
    #[test]
    fn the_total_is_the_sum_of_lines_and_tax(lines in lines(), business in any::<bool>()) {
        let s = settings();
        let group = if business { "business" } else { "consumer" };
        let net = s.is_business(group);
        let products: Vec<Product> = lines.iter().enumerate().map(|(n, (price, rate, _))| product(n, *price, *rate, 1, 1)).collect();
        let items: Vec<Value> = lines.iter().enumerate().map(|(n, (_, _, q))| json!({"id":format!("p{n}"),"quantity":q})).collect();
        let c = cart(json!(items), group);
        let q = quote(&c, &products, &s).unwrap();
        let price = &q["price"];
        let line_sum: i64 = q["lineItems"].as_array().unwrap().iter().map(|l| cents(&l["price"]["totalPrice"])).sum();
        let tax_sum: i64 = q["lineItems"].as_array().unwrap().iter().map(|l| cents(&l["price"]["calculatedTaxes"].as_array().unwrap().iter().map(|t| t["tax"].as_f64().unwrap()).sum::<f64>().into())).sum();
        let (position, total, tax, net_price) = (cents(&price["positionPrice"]), cents(&price["totalPrice"]), cents(&price["tax"]), cents(&price["netPrice"]));
        prop_assert_eq!(position, line_sum);
        prop_assert_eq!(tax, tax_sum);
        prop_assert!(tax >= 0);
        if net {
            prop_assert_eq!(total, position + tax);
            prop_assert_eq!(net_price, position);
        } else {
            prop_assert_eq!(total, position);
            prop_assert!(tax <= total);
            prop_assert_eq!(net_price + tax, total);
        }
        for (line, (_, _, quantity)) in q["lineItems"].as_array().unwrap().iter().zip(&lines) {
            prop_assert_eq!(cents(&line["price"]["totalPrice"]), cents(&line["price"]["unitPrice"]) * i64::from(*quantity));
        }
        prop_assert_eq!(quote(&c, &products, &s).unwrap(), q);
    }

    /// An allowed quantity is within the product's minimum and the maximum, on its step grid,
    /// and normalizing it again changes nothing.
    #[test]
    fn normalized_quantities_are_on_the_grid(min in 1_u32..20, steps in 1_u32..10, requested in 0_u32..20_000) {
        let p = product(0, 100, 19., min, steps);
        let q = normalized_quantity(&p, requested).unwrap();
        prop_assert!(q >= min && q <= 10_000);
        prop_assert_eq!((q - min) % steps, 0);
        prop_assert_eq!(normalized_quantity(&p, q).unwrap(), q);
    }
}
