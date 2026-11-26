//! Tests that kill the mutants `cargo-mutants` found surviving in checkout `domain/`: exact quote amounts for consumer and business carts, tiers, list/regulation/reference prices, app fees, and the review and tax-line rules.
use serde_json::{Value, json};
use veyndo_checkout::domain::cart_quote::{normalized_quantity, quote};
use veyndo_checkout::domain::quote_view::digital;
use veyndo_checkout::domain::session_review::Review;
use veyndo_checkout::domain::tax_lines::tax_products;
use veyndo_pricing::catalog::Product;
use veyndo_shop::Settings;
use veyndo_shop::types::cart::StoredCart;
use veyndo_shop::types::settings::CheckoutSelection;

fn settings() -> Settings {
    serde_json::from_str(include_str!("../../../fixtures/demo-settings.json")).unwrap()
}

fn product(id: &str, extra: Value) -> Product {
    let mut p = json!({
        "extra":{},"id":id,"name":"Lamp","category":"","description":"","price":119.0,
        "tax_rate":19.0,"stock":5,"revision":1,"list_price":null,"regulation_price":null,
        "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
        "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},"delivery_days":1
    });
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    serde_json::from_value(p).unwrap()
}

fn cart(group: &str, items: Value, configurations: Value) -> StoredCart {
    StoredCart {
        id: "c1".into(),
        tenant: "demo".into(),
        token: "t".into(),
        data: serde_json::from_value(
            json!({"items":items,"group":group,"email":null,"company":null,
            "session":"s","buyer":null,"order":null,"app_configurations":configurations}),
        )
        .unwrap(),
        revision: 1,
        status: "open".into(),
    }
}

fn priced(group: &str, p: Product, configurations: Value) -> Value {
    quote(
        &cart(group, json!([{"id":"p1","quantity":2}]), configurations),
        &[p],
        &settings(),
    )
    .unwrap()
}

fn fee(product: &str, currency: Option<&str>) -> Value {
    fee_of(product, currency, 250)
}

fn fee_of(product: &str, currency: Option<&str>, minor: i64) -> Value {
    let mut c = json!({"app":"engraving","productId":product,"fields":{},"feeMinor":minor,"ruleRevision":1,
        "appVersion":"1.0.0","entity":"engravings","record":"r1"});
    if let Some(code) = currency {
        c["currency"] = json!(code);
    }
    c
}

#[test]
fn a_consumer_quote_has_exact_gross_amounts() {
    let q = priced("consumer", product("p1", json!({})), json!({}));
    let line = &q["lineItems"][0]["price"];
    assert_eq!(
        (line["unitPrice"].as_f64(), line["totalPrice"].as_f64()),
        (Some(119.), Some(238.))
    );
    assert_eq!(line["calculatedTaxes"][0]["tax"], json!(38.0));
    let price = &q["price"];
    assert_eq!(
        [
            &price["positionPrice"],
            &price["totalPrice"],
            &price["tax"],
            &price["netPrice"]
        ],
        [&json!(238.0), &json!(238.0), &json!(38.0), &json!(200.0)]
    );
}

#[test]
fn a_business_quote_prices_net_and_adds_the_tax() {
    let q = priced(
        "business",
        product("p1", json!({"list_price":150.0,"regulation_price":140.0})),
        json!({}),
    );
    let line = &q["lineItems"][0]["price"];
    assert_eq!(
        (line["unitPrice"].as_f64(), line["totalPrice"].as_f64()),
        (Some(100.), Some(200.))
    );
    assert_eq!(line["listPrice"]["price"], json!(126.05));
    assert_eq!(line["regulationPrice"]["price"], json!(117.65));
    let price = &q["price"];
    assert_eq!(price["taxStatus"], "net");
    assert_eq!(
        [
            &price["positionPrice"],
            &price["totalPrice"],
            &price["tax"],
            &price["netPrice"]
        ],
        [&json!(200.0), &json!(238.0), &json!(38.0), &json!(200.0)]
    );
}

#[test]
fn tiers_list_regulation_and_reference_prices_reach_the_line() {
    let p = product(
        "p1",
        json!({"list_price":150.0,"regulation_price":140.0,
        "reference_price":{"purchase_unit":0.5,"reference_unit":1.0,"unit_name":"l"},
        "advanced_prices":[{"rule_id":"consumer","quantity_start":1,"quantity_end":null,"discount":0.1}]}),
    );
    let q = priced("consumer", p, json!({}));
    let line = &q["lineItems"][0];
    assert_eq!(line["price"]["unitPrice"], json!(107.1));
    assert_eq!(line["price"]["totalPrice"], json!(214.2));
    assert_eq!(line["discountPercent"], json!(10.0));
    assert_eq!(line["price"]["listPrice"]["price"], json!(150.0));
    assert_eq!(line["price"]["regulationPrice"]["price"], json!(140.0));
    assert_eq!(line["price"]["referencePrice"]["price"], json!(214.2));
}

#[test]
fn an_app_fee_is_priced_per_unit_on_its_own_line_only() {
    for currency in [None, Some("EUR")] {
        // Another product's (larger) fee never reaches this line.
        let configs = json!({"a": fee("p1", currency), "b": fee_of("p2", currency, 1000)});
        let q = priced("consumer", product("p1", json!({})), configs.clone());
        let line = &q["lineItems"][0];
        assert_eq!(line["price"]["unitPrice"], json!(121.5), "{currency:?}");
        assert_eq!(line["price"]["totalPrice"], json!(243.0));
        assert_eq!(
            line["price"]["calculatedTaxes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["tax"].as_f64().unwrap())
                .sum::<f64>(),
            38.8
        );
        assert_eq!(line["appConfigurations"].as_array().unwrap().len(), 1);
        assert_eq!(q["price"]["tax"], json!(38.8));
        let business = priced("business", product("p1", json!({})), configs);
        assert_eq!(business["lineItems"][0]["price"]["unitPrice"], json!(102.1));
        assert_eq!(
            business["lineItems"][0]["price"]["totalPrice"],
            json!(204.2)
        );
        // The fee is priced net for business groups: 2.10 x 19 % x 2 = 0.80 on top of 38.00.
        assert_eq!(business["price"]["tax"], json!(38.8));
    }
    let other = priced(
        "consumer",
        product("p1", json!({})),
        json!({"b": fee("p2", None)}),
    );
    assert!(other["lineItems"][0].get("configuration").is_none());
}

#[test]
fn a_maximum_equal_to_the_minimum_still_allows_that_quantity() {
    let p = product("p1", json!({"min_purchase":3,"max_purchase":3}));
    assert_eq!(normalized_quantity(&p, 1).unwrap(), 3);
}

#[test]
fn only_a_true_digital_flag_skips_shipping() {
    assert!(digital(&json!({"digital":true})));
    assert!(!digital(&json!({"digital":false})));
    assert!(!digital(&json!({})));
}

#[test]
fn a_changed_cart_id_alone_needs_a_new_review() {
    let review: Review = serde_json::from_value(json!({"sessionId":"3f2b8c1e-5a4d-4e6f-8a9b-0c1d2e3f4a5b",
        "cartId":"cart-1","customerId":"customer-1","cartRevision":3,"locale":"de-DE","currency":"EUR",
        "country":"DE","paymentMethod":"simulated","legalPolicyVersion":"policy-7","totalMinor":"1000",
        "confirmed":true,"paypalSandbox":false})).unwrap();
    let mut c = cart("retail", json!([]), json!({}));
    c.data = serde_json::from_value(json!({"items":[],"group":"retail","session":"s","customer_id":"customer-1","locale":"de-DE"})).unwrap();
    c.id = "cart-1".into();
    assert!(
        review
            .admit_cart(&c, &c.data.sales_channel.clone(), "customer-1")
            .is_ok()
    );
    c.id = "cart-2".into();
    assert!(
        review
            .admit_cart(&c, &c.data.sales_channel.clone(), "customer-1")
            .is_err()
    );
}

#[test]
fn foreign_rates_convert_fixed_currency_and_regulation_prices() {
    let mut selection = CheckoutSelection::defaults();
    selection.country = "AT".into();
    let p = product(
        "p1",
        json!({"regulation_price":119.0,
        "extra":{"currencyPrices":{"EUR":{"price":"119.00"},"XXX":{"price":"119.00"}}}}),
    );
    let abroad = tax_products(&[p], &selection, &settings()).unwrap();
    assert!((abroad[0].regulation_price.unwrap() - 120.).abs() < 1e-9);
    assert_eq!(
        abroad[0].extra["currencyPrices"]["EUR"]["price"],
        json!("120.00")
    );
    assert_eq!(
        abroad[0].extra["currencyPrices"]["XXX"]["price"],
        json!("119.00")
    );
}

#[test]
fn a_reduced_rate_product_and_its_fee_use_the_product_rate() {
    let q = priced(
        "consumer",
        product("p1", json!({"price":107.0,"tax_rate":7.0})),
        json!({"a": fee("p1", None)}),
    );
    // (107.00 + 2.50) x 2 = 219.00 gross, of which 7 %: 14.00 + 0.33 = 14.33.
    assert_eq!(q["lineItems"][0]["price"]["totalPrice"], json!(219.0));
    assert_eq!(q["price"]["tax"], json!(14.33));
}

#[test]
fn a_whole_unit_currency_rounds_lines_and_fees_to_whole_units() {
    let mut s = settings();
    s.currencies.enabled.push("JPY".into());
    s.currencies.definitions.push(
        serde_json::from_value(
            json!({"code":"JPY","scale":0,"rate":"150.00000000","strategy":"automatic"}),
        )
        .unwrap(),
    );
    let mut c = cart(
        "consumer",
        json!([{"id":"p1","quantity":1}]),
        json!({"a": fee("p1", None)}),
    );
    c.data.currency = "JPY".into();
    let q = quote(&c, &[product("p1", json!({"price":1.19}))], &s).unwrap();
    // 1.19 EUR = 178.5 JPY -> 179; the 2.50 EUR fee = 375 JPY.
    assert_eq!(q["price"]["currency"], "JPY");
    assert_eq!(q["lineItems"][0]["price"]["unitPrice"], json!(554.0));
    // Taxes round to whole yen per entry: 179 / 1.19 x 0.19 = 28.58 -> 29, 375 / 1.19 x 0.19 = 59.87 -> 60.
    let taxes: Vec<f64> = q["lineItems"][0]["price"]["calculatedTaxes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tax"].as_f64().unwrap())
        .collect();
    assert_eq!(taxes, [29., 60.]);
}
