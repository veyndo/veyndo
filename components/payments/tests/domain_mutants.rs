//! Tests that kill the mutants `cargo-mutants` found surviving in payments `domain/`: each pins one boundary or clause of amounts, transaction details, instructions, receipts and provider URLs.
use serde_json::{Value, json};
use veyndo_core::{PortFuture, Result, bad};
use veyndo_payments::domain::amounts::parse_amount;
use veyndo_payments::domain::attempt::Attempt;
use veyndo_payments::domain::details_model::Details;
use veyndo_payments::domain::diagnostics::code;
use veyndo_payments::domain::instructions::Instructions;
use veyndo_payments::domain::receipt_ref::ProviderReceipt;
use veyndo_payments::domain::return_urls::build;
use veyndo_payments::domain::service_config::{approval_url, validate_service};
use veyndo_payments::ports::Host;

fn attempt() -> Attempt {
    Attempt {
        id: "a".into(),
        provider: "example".into(),
        context: json!({}),
        tenant: "shop".into(),
        order: "o".into(),
        amount: 1000,
        currency: "EUR".into(),
        currency_scale: 2,
        state: "captured".into(),
        provider_order: Some("ORDER".into()),
        capture: Some("CAP".into()),
        refunded: 0,
        adapter_version: "1.4.0".into(),
        environment: "sandbox".into(),
        bn_code: "".into(),
    }
}

#[test]
fn a_zero_provider_amount_is_valid() {
    assert_eq!(parse_amount("0.00", "EUR").unwrap(), 0);
    assert!(parse_amount("-0.01", "EUR").is_err());
}

fn entry(id: &str, fees: Value) -> Value {
    json!({"id":id,"status":"COMPLETED","amountMinor":1000,"currency":"EUR","currencyScale":2,"fees":fees})
}

fn details(captures: Vec<Value>) -> bool {
    let v = json!({"reference":"ORDER","status":"COMPLETED","captures":captures,"authorizations":[],"refunds":[]});
    serde_json::from_value::<Details>(v).is_ok_and(|d| d.validate(&attempt()).is_ok())
}

fn fee(minor: i64) -> Value {
    json!({"kind":"provider","amount":{"amountMinor":minor,"currency":"EUR","currencyScale":2}})
}

#[test]
fn fee_amounts_admit_zero_and_the_exact_integer_limit() {
    assert!(details(vec![entry("CAP", json!([fee(0)]))]));
    assert!(details(vec![entry(
        "CAP",
        json!([fee(9_007_199_254_740_991)])
    )]));
    assert!(!details(vec![entry(
        "CAP",
        json!([fee(9_007_199_254_740_992)])
    )]));
    assert!(!details(vec![entry("CAP", json!([fee(-1)]))]));
}

#[test]
fn entries_and_fees_admit_exactly_their_limits() {
    assert!(details(vec![entry(
        "CAP",
        json!((0..32).map(|_| fee(1)).collect::<Vec<_>>())
    )]));
    assert!(!details(vec![entry(
        "CAP",
        json!((0..33).map(|_| fee(1)).collect::<Vec<_>>())
    )]));
    let rows = |n: usize| {
        (0..n)
            .map(|i| entry(&format!("CAP{i}"), json!([])))
            .collect::<Vec<_>>()
    };
    assert!(details(rows(1000)));
    assert!(!details(rows(1001)));
}

#[test]
fn every_listed_diagnostic_code_passes_through() {
    for c in [
        "PAYMENT_SAVED_METHODS_UNAVAILABLE",
        "PAYMENT_METHOD_UNAVAILABLE",
        "PAYMENT_PROVIDER_CONFIGURATION_REQUIRED",
    ] {
        assert_eq!(code(Some(c)), Some(c));
    }
    assert_eq!(code(Some("OTHER")), None);
}

fn instructions(change: impl FnOnce(&mut Value)) -> bool {
    let mut v = json!({"kind":"bank_transfer","paymentReference":"RAC-1 ref","iban":"DE89370400440532013000",
        "bic":"COBADEFFXXX","bankName":"Bank","accountHolderName":"Shop GmbH"});
    change(&mut v);
    serde_json::from_value::<Instructions>(v).is_ok_and(|i| i.validate().is_ok())
}

#[test]
fn recipient_instructions_check_each_field_on_its_own() {
    assert!(instructions(|_| {}));
    assert!(instructions(|v| v["bankName"] = json!("b".repeat(200))));
    // IBAN lengths 15 and 34 with valid checksums are the bounds.
    assert!(instructions(|v| v["iban"] = json!("NO9386011117947")));
    assert!(instructions(
        |v| v["iban"] = json!("DE75111111111111111111111111111111")
    ));
    for refuse in [
        (|v: &mut Value| v["iban"] = json!("DE111111111111111111111111111111111"))
            as fn(&mut Value),
        |v| v["iban"] = json!("NO938601111794"),
        |v| v["iban"] = json!("de89370400440532013000"),
        // Valid checksums, so only the country-letter and check-digit rules refuse them.
        |v| v["iban"] = json!("1E893704004405320100067"),
        |v| v["iban"] = json!("DEA93704004405320100031"),
        |v| v["bankName"] = json!("   "),
        |v| v["bankName"] = json!("b".repeat(201)),
        |v| v["bankName"] = json!("Bank\u{7}"),
        |v| v["accountHolderName"] = json!(""),
        |v| v["paymentReference"] = json!("ref#1"),
        |v| v["bic"] = json!("COBADE"),
    ] {
        assert!(!instructions(refuse));
    }
}

#[test]
fn only_an_explicit_true_confirms_a_receipt() {
    let confirmed = |v: Value| {
        serde_json::from_value::<ProviderReceipt>(v)
            .unwrap()
            .confirmed()
    };
    assert!(confirmed(json!({"confirmed":true})));
    assert!(!confirmed(json!({"confirmed":false})));
    assert!(!confirmed(json!({})));
}

#[test]
fn return_urls_refuse_each_unsafe_origin() {
    assert!(build("https://shop.example.test", "t", "c", "a", true).is_ok());
    for origin in [
        "ftp://shop.example.test",
        "https://user@shop.example.test",
        "https://:pw@shop.example.test",
    ] {
        assert!(build(origin, "t", "c", "a", false).is_err(), "{origin}");
    }
    assert!(build("http://shop.example.test", "t", "c", "a", true).is_err());
}

#[test]
fn service_configuration_rules() {
    let token = "t".repeat(24);
    let service =
        |url: &str, environment: &str| json!({"url":url,"environment":environment,"token":token});
    assert!(validate_service(&service("https://localhost/pay", "sandbox"), "sandbox").is_ok());
    assert!(
        validate_service(
            &service("http://example.test/pay", "contract-fixture"),
            "contract-fixture"
        )
        .is_err()
    );
    assert!(
        validate_service(
            &service("https://user@pay.example.test/", "sandbox"),
            "sandbox"
        )
        .is_err()
    );
}

struct Origins;
impl Host for Origins {
    fn service(&self, _p: &str, _v: &str, _e: &str) -> Result<Value> {
        Ok(json!({"approvalOrigins":["https://pay.example.test"]}))
    }
    fn captured<'a, 'c: 'a>(
        &'a self,
        _tx: &'a mut sqlx::Transaction<'c, sqlx::Postgres>,
        _tenant: &'a str,
        _order: &'a mut Value,
        _state: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Err(bad("not used")) })
    }
}

#[test]
fn approval_urls_refuse_credentials_in_the_url() {
    let url = |u: &str| approval_url(&Origins, &attempt(), &json!(u));
    assert_eq!(
        url("https://pay.example.test/approve").unwrap().as_deref(),
        Some("https://pay.example.test/approve")
    );
    assert!(url("https://user@pay.example.test/approve").is_err());
    assert!(url("https://:pw@pay.example.test/approve").is_err());
}

/// Carts mirror no order snapshot and no experiment is rewarded.
impl veyndo_payments::ports::LedgerRecords for Origins {
    fn mirror_cart_order<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a str,
        _: &'a serde_json::Value,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn reward_capture<'a>(
        &'a self,
        _: &'a mut sqlx::PgConnection,
        _: &'a str,
        _: &'a str,
    ) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
