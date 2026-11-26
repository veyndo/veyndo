//! Tests that kill the mutants `cargo-mutants` found surviving in pricing `domain/`: each pins one boundary or clause.
use veyndo_pricing::catalog::Tier;
use veyndo_pricing::customer::{Address, Contact};
use veyndo_pricing::domain::context::{fix_quantity, select_tier};
use veyndo_pricing::money::{Currency, Money};

fn eur() -> Currency {
    Currency::eur()
}

#[test]
fn a_whole_currency_refuses_a_trailing_point() {
    let whole = Currency::new("JPY", 0).unwrap();
    assert!(Money::parse("5.", whole.clone()).is_err());
    assert_eq!(Money::parse("5", whole).unwrap().minor(), 5);
}

#[test]
fn rounding_reads_at_most_sixty_four_characters() {
    let at_limit = format!("0.{}", "0".repeat(62));
    assert_eq!(at_limit.len(), 64);
    assert_eq!(Money::rounded(&at_limit, eur()).unwrap().minor(), 0);
    assert!(Money::rounded(&format!("{at_limit}0"), eur()).is_err());
}

#[test]
fn rounding_refuses_each_malformed_part_as_a_decimal_error() {
    for value in [".5", "1a.5", "1.5a", "1a", "a"] {
        assert_eq!(
            Money::rounded(value, eur()),
            Err("Decimal amount required"),
            "{value}"
        );
    }
}

#[test]
fn zero_has_no_sign() {
    assert_eq!(Money::new(0, eur()).decimal(), "0.00");
    assert_eq!(Money::new(-1, eur()).decimal(), "-0.01");
}

#[test]
fn a_tier_is_chosen_from_the_first_matching_rule_only() {
    let tier = |rule: &str, start, end, discount| Tier {
        rule_id: rule.into(),
        quantity_start: start,
        quantity_end: end,
        discount,
    };
    let tiers = [
        tier("vip", 1, Some(9), 0.1),
        tier("other", 1, None, 0.5),
        tier("vip", 10, None, 0.2),
    ];
    let rules = ["vip".to_string()];
    assert_eq!(select_tier(&tiers, &rules, 5).unwrap().discount, 0.1);
    assert_eq!(select_tier(&tiers, &rules, 50).unwrap().discount, 0.2);
    // A rule without tiers is skipped in favour of the next rule that has some.
    let ordered = ["missing".to_string(), "vip".to_string()];
    assert_eq!(select_tier(&tiers, &ordered, 5).unwrap().discount, 0.1);
}

#[test]
fn quantities_snap_down_onto_the_step_grid() {
    assert_eq!(fix_quantity(2, 7, 2), 6);
    assert_eq!(fix_quantity(1, 1, 1), 1);
    assert_eq!(fix_quantity(3, 10, 4), 7);
}

fn address() -> Address {
    serde_json::from_value(
        serde_json::json!({"name":"A","street":"S","postalCode":"1","city":"C","country":"DE"}),
    )
    .unwrap()
}

#[test]
fn address_fields_admit_exactly_one_hundred_sixty_characters() {
    let mut a = address();
    a.street = "s".repeat(160);
    a.company = "c".repeat(160);
    assert!(a.clone().validate().is_ok());
    let mut long = a.clone();
    long.street = "s".repeat(161);
    assert!(long.validate().is_err());
    let mut long = a;
    long.company = "c".repeat(161);
    assert!(long.validate().is_err());
}

fn contact() -> Contact {
    Contact {
        name: "Buyer".into(),
        ..Contact::default()
    }
}

#[test]
fn contact_limits_are_inclusive() {
    let mut c = contact();
    c.name = "n".repeat(100);
    c.first_name = "f".repeat(200);
    c.vat_ids = (0..10).map(|_| "v".repeat(50)).collect();
    assert!(c.validate().is_ok());
    for change in [
        |c: &mut Contact| c.name = "n".repeat(101),
        |c: &mut Contact| c.first_name = "f".repeat(201),
        |c: &mut Contact| c.vat_ids = (0..11).map(|_| "v".into()).collect(),
        |c: &mut Contact| c.vat_ids = vec!["v".repeat(51)],
    ] {
        let mut c = contact();
        change(&mut c);
        assert!(c.validate().is_err());
    }
}

#[test]
fn a_birthday_needs_both_dashes_and_digits() {
    for (birthday, valid) in [
        ("1990-01-31", true),
        ("1990/01-31", false),
        ("1990-01/31", false),
        ("1990-0a-31", false),
    ] {
        let mut c = contact();
        c.birthday = Some(birthday.into());
        assert_eq!(c.validate().is_ok(), valid, "{birthday}");
    }
}

mod calculation {
    use serde_json::{Value, json};
    use veyndo_pricing::decimal::Decimal;
    use veyndo_pricing::pricing::{PriceInput, calculate};

    /// The calculated price for a wire input (JSON numbers, as the price fixture sends them).
    fn calc(fields: Value) -> Value {
        let mut input = json!({"price": 0.0, "quantity": 1, "tax_rate": 19.0});
        for (key, value) in fields.as_object().unwrap() {
            input[key] = value.clone();
        }
        let input: PriceInput = serde_json::from_value(input).unwrap();
        serde_json::to_value(calculate(&input).unwrap()).unwrap()
    }

    #[test]
    fn omitted_fields_default_to_gross_calculated_cents() {
        let i: PriceInput =
            serde_json::from_value(json!({"price":1.0,"quantity":1,"tax_rate":19.0})).unwrap();
        assert!(i.gross && i.calculated && !i.round_for_net);
        assert_eq!(
            (i.decimals, i.interval.decimal().unwrap()),
            (2, Decimal::parse("0.01").unwrap())
        );
    }

    #[test]
    fn a_later_rule_with_the_same_rate_replaces_the_earlier_one() {
        let price = calc(json!({"price": 119.0, "tax_rules": [
            {"tax_rate": 19.0, "percentage": 50.0}, {"tax_rate": 19.0, "percentage": 100.0}]}));
        assert_eq!(price["calculated_taxes"].as_array().unwrap().len(), 1);
        assert_eq!(price["calculated_taxes"][0]["tax"], 19.0);
    }

    #[test]
    fn net_prices_round_to_the_cash_interval_only_when_asked() {
        let net = |round_for_net: bool| {
            calc(json!({"price": 1.02, "gross": false, "round_for_net": round_for_net, "interval": 0.05}))
                ["unit_price"]
                .clone()
        };
        assert_eq!(net(true), 1.0);
        assert_eq!(net(false), 1.02);
    }

    #[test]
    fn an_uncalculated_net_price_gets_each_rule_tax_added() {
        let gross = |rules: Value| {
            calc(json!({"price": 100.0, "calculated": false, "tax_rules": rules}))["unit_price"]
                .clone()
        };
        assert_eq!(gross(Value::Null), 119.0);
        assert_eq!(
            gross(
                json!([{"tax_rate": 19.0, "percentage": 50.0}, {"tax_rate": 7.0, "percentage": 50.0}])
            ),
            113.0
        );
        // A rule tax at the 1e-8 zero threshold still counts; one below it is noise.
        let tiny = |price: f64| {
            calc(json!({"price": price, "calculated": false, "tax_rate": 100.0, "decimals": 10}))
                ["unit_price"]
                .clone()
        };
        assert_eq!(tiny(1e-8), 2e-8);
        assert_eq!(tiny(1e-9), 1e-9);
    }

    #[test]
    fn net_line_taxes_and_rule_prices_scale_with_the_quantity() {
        let price = calc(json!({"price": 10.0, "gross": false, "quantity": 3}));
        assert_eq!(price["calculated_taxes"][0]["tax"], 5.7);
        assert_eq!(price["calculated_taxes"][0]["price"], 30.0);
        assert_eq!(price["total_price"], 30.0);
    }

    #[test]
    fn a_list_price_states_its_discount_and_percentage() {
        let list = calc(json!({"price": 120.0, "list_price": 150.0}))["list_price"].clone();
        assert_eq!(
            list,
            json!({"price": 150.0, "discount": -30.0, "percentage": 20.0})
        );
        let net =
            calc(json!({"price": 100.0, "calculated": false, "list_price": 200.0}))["list_price"]
                .clone();
        assert_eq!(
            (net["price"].clone(), net["discount"].clone()),
            (json!(238.0), json!(-119.0))
        );
        for absent in [json!(0.0), json!(0.001), json!(-5.0), Value::Null] {
            assert!(
                calc(json!({"price": 120.0, "list_price": absent}))["list_price"].is_null(),
                "{absent:?}"
            );
        }
    }

    #[test]
    fn a_regulation_price_is_rounded_like_the_unit() {
        assert_eq!(
            calc(json!({"price": 1.0, "regulation_price": 99.999}))["regulation_price"],
            100.0
        );
        assert_eq!(
            calc(json!({"price": 1.0, "calculated": false, "regulation_price": 100.0}))["regulation_price"],
            119.0
        );
        assert!(calc(json!({"price": 1.0, "regulation_price": 0.0}))["regulation_price"].is_null());
    }

    #[test]
    fn a_reference_price_needs_both_positive_units() {
        let reference = |purchase_unit: f64, reference_unit: f64| {
            calc(
                json!({"price": 3.0, "reference": {"purchase_unit": purchase_unit,
                "reference_unit": reference_unit, "unit_name": "l"}}),
            )["reference_price"]["price"]
                .clone()
        };
        assert_eq!(reference(0.5, 2.), 12.0);
        assert!(reference(0., 1.).is_null());
        assert!(reference(1., 0.).is_null());
        assert!(reference(-1., -1.).is_null());
    }
}

mod external_orders {
    use serde_json::{Value, json};
    use veyndo_pricing::external_order::PaidOrder;

    fn order() -> Value {
        let address = json!({"name":"A","street":"S","postalCode":"1","city":"C","country":"DE"});
        json!({"salesChannelId":"web","externalId":"ext-1","currency":"EUR","locale":"de-DE","email":"a@example.test",
            "billingAddress":address,"shippingAddress":address,
            "lines":[{"productId":"lamp","quantity":1,"amount":{"grossMinor":1190,"taxMinor":190,"taxRateBasisPoints":1900}}],
            "shipping":{"grossMinor":0,"taxMinor":0,"taxRateBasisPoints":0},"totalMinor":1190,"paid":true})
    }

    fn valid(change: impl FnOnce(&mut Value)) -> bool {
        let mut v = order();
        change(&mut v);
        serde_json::from_value::<PaidOrder>(v)
            .unwrap()
            .validate()
            .is_ok()
    }

    fn lines(n: usize) -> Value {
        json!((0..n).map(|i| json!({"productId":format!("p{i}"),"quantity":1,"amount":{"grossMinor":10,"taxMinor":0,"taxRateBasisPoints":0}})).collect::<Vec<_>>())
    }

    #[test]
    fn each_header_rule_refuses_on_its_own() {
        assert!(valid(|_| {}));
        assert!(valid(|v| v["salesChannelId"] = json!("c".repeat(100))));
        assert!(valid(|v| v["externalId"] = json!("e".repeat(100))));
        assert!(valid(
            |v| v["email"] = json!(format!("{}@x", "a".repeat(252)))
        ));
        assert!(valid(|v| {
            v["lines"] = lines(24);
            v["totalMinor"] = json!(240);
        }));
        for refuse in [
            (|v: &mut Value| v["salesChannelId"] = json!("")) as fn(&mut Value),
            |v| v["salesChannelId"] = json!("c".repeat(101)),
            |v| v["externalId"] = json!(""),
            |v| v["externalId"] = json!("e".repeat(101)),
            |v| v["externalId"] = json!("ext\u{7}"),
            |v| v["lines"] = json!([]),
            |v| {
                v["lines"] = lines(25);
                v["totalMinor"] = json!(250);
            },
            |v| v["email"] = json!(format!("{}@x", "a".repeat(253))),
            |v| v["email"] = json!("no-at-sign"),
            |v| v["email"] = json!("a@x\u{7}"),
            |v| v["shipping"]["taxMinor"] = json!(5),
        ] {
            assert!(!valid(refuse));
        }
    }

    #[test]
    fn each_line_rule_refuses_on_its_own() {
        assert!(valid(
            |v| v["lines"][0]["productId"] = json!("p".repeat(100))
        ));
        for refuse in [
            (|v: &mut Value| v["lines"][0]["quantity"] = json!(0)) as fn(&mut Value),
            |v| v["lines"][0]["productId"] = json!(""),
            |v| v["lines"][0]["productId"] = json!("p".repeat(101)),
            |v| v["lines"][0]["amount"]["taxMinor"] = json!(2000),
            |v| {
                v["lines"] = json!([v["lines"][0].clone(), v["lines"][0].clone()]);
                v["totalMinor"] = json!(2380);
            },
        ] {
            assert!(!valid(refuse));
        }
    }
}
