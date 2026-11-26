//! Tests that kill the mutants `cargo-mutants` found surviving in orders `domain/`: machine limits and per-clause validation, company data, logo bounds, delivery tracking and import identities.
use image::{DynamicImage, ImageFormat, RgbImage};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Cursor;
use veyndo_orders::domain::company_model::{resolve, validate};
use veyndo_orders::domain::import_relations::identity;
use veyndo_orders::domain::logo_image::normalize;
use veyndo_orders::domain::order_machine::{
    MachineEdge, MachineState, OrderMachine, OrderMachineValidation, default_machine,
};
use veyndo_orders::domain::order_transitions::delivery;
use veyndo_orders::types::source_record::SourceRecord;
use veyndo_shop::Settings;

fn labels(text: &str) -> HashMap<String, String> {
    ["en", "de", "fr", "es"]
        .into_iter()
        .map(|l| (l.to_string(), text.to_string()))
        .collect()
}

fn state(id: &str) -> MachineState {
    MachineState {
        id: id.into(),
        label: labels("State"),
        terminal: false,
    }
}

fn edge(id: &str, from: &str, to: &str) -> MachineEdge {
    MachineEdge {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        label: labels("Edge"),
        policy: None,
    }
}

/// The default machine plus `extra` states, each entered from `placed`.
fn extended(extra: usize) -> OrderMachine {
    let mut m = default_machine();
    for n in 0..extra {
        m.states.push(state(&format!("extra_{n}")));
        m.transitions
            .push(edge(&format!("enter_{n}"), "placed", &format!("extra_{n}")));
    }
    m
}

#[test]
fn a_machine_holds_at_most_32_states_and_100_transitions() {
    assert!(extended(28).validate().is_ok());
    assert!(extended(29).validate().is_err());
    let full = extended_with(100);
    assert_eq!((full.states.len(), full.transitions.len()), (32, 100));
    assert!(full.validate().is_ok());
    assert!(extended_with(101).validate().is_err());
}

/// The 32-state machine with `count` transitions in total (unique pairs between the added states).
fn extended_with(count: usize) -> OrderMachine {
    let mut m = extended(28);
    'fill: for i in 0..28 {
        for j in 0..28 {
            if m.transitions.len() >= count {
                break 'fill;
            }
            if i != j {
                m.transitions.push(edge(
                    &format!("l_{i}_{j}"),
                    &format!("extra_{i}"),
                    &format!("extra_{j}"),
                ));
            }
        }
    }
    m
}

#[test]
fn every_state_and_edge_rule_refuses_on_its_own() {
    let base = || extended(1);
    assert!(base().validate().is_ok());
    let cases: Vec<fn(&mut OrderMachine)> = vec![
        |m| m.states[4].label = labels(" "),
        |m| m.states[4].label = labels(&"x".repeat(101)),
        |m| m.states[4].id = "Not An Identifier".into(),
        |m| m.states.push(state("extra_0")),
        |m| m.transitions[4].label = labels(""),
        |m| m.transitions[4].to = "nowhere".into(),
        |m| m.transitions.push(edge("loop", "extra_0", "extra_0")),
        |m| {
            m.transitions
                .push(edge("enter_0", "in_progress", "extra_0"))
        },
    ];
    for change in cases {
        let mut m = base();
        change(&mut m);
        assert!(m.validate().is_err());
    }
    let mut m = base();
    m.states[4].label = labels(&"x".repeat(100));
    assert!(m.validate().is_ok());
}

#[test]
fn tracking_codes_admit_one_hundred_characters_and_only_safe_links() {
    let ship = |input: Value| {
        delivery(
            &mut json!({"deliveries":[{"state":"open","booking":null}]}),
            &input,
            "shipped",
        )
    };
    assert!(ship(json!({"trackingCode":"t".repeat(100)})).is_ok());
    assert!(ship(json!({"trackingCode":"t".repeat(101)})).is_err());
    assert!(ship(json!({"trackingUrl":"https://user:pw@carrier.example/1"})).is_err());
}

fn settings() -> Settings {
    serde_json::from_str(include_str!("../../../fixtures/demo-settings.json")).unwrap()
}

fn company(extra: Value) -> Value {
    let mut v = json!({"name":"Shop GmbH","street":"Main 1","city":"Berlin","country":"DE","postalCode":"10115"});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn company_text_limits_and_urls() {
    let s = settings();
    let ok = |extra: Value| validate(&company(extra), &s, false).is_ok();
    assert!(ok(json!({})));
    assert!(ok(json!({"brandName":{"en-GB":"b".repeat(500)}})));
    assert!(!ok(json!({"brandName":{"en-GB":"b".repeat(501)}})));
    assert!(ok(json!({"taxId":"t".repeat(500)})));
    assert!(!ok(json!({"taxId":"t".repeat(501)})));
    assert!(!ok(json!({"country":"XX"})));
    assert!(!ok(json!({"website":"https://user@shop.example"})));
    assert!(!ok(json!({"website":"https://:pw@shop.example"})));
    assert!(ok(json!({"website":"https://shop.example"})));
}

#[test]
fn overrides_merge_only_localized_texts() {
    let base = json!({"name":{"de":"A"},"brandName":{"de-DE":"Marke"}});
    let merged = resolve(
        &base,
        &json!({"name":{"en":"B"},"brandName":{"en-GB":"Brand"}}),
    );
    assert_eq!(merged["name"], json!({"en":"B"}));
    assert_eq!(
        merged["brandName"],
        json!({"de-DE":"Marke","en-GB":"Brand"})
    );
    assert_eq!(
        resolve(&base, &json!({"brandName":"Plain"}))["brandName"],
        json!("Plain")
    );
}

fn record() -> SourceRecord {
    SourceRecord {
        version: 1,
        source: "shopware".into(),
        source_shop: "old-shop".into(),
        kind: "products".into(),
        source_id: "p1".into(),
        data: json!({}),
        warnings: vec![],
    }
}

#[test]
fn source_identities_admit_two_hundred_characters() {
    assert!(identity(&record(), "shop", "products", &"s".repeat(200)).is_ok());
    assert!(identity(&record(), "shop", "products", &"s".repeat(201)).is_err());
}

/// Deterministic noise an encoder cannot compress well.
fn noise(side: u32, format: ImageFormat) -> Vec<u8> {
    let mut seed = 0x2545_f491_u32;
    let img = RgbImage::from_fn(side, side, |_, _| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        image::Rgb([seed as u8, (seed >> 8) as u8, (seed >> 16) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(img)
        .write_to(&mut out, format)
        .unwrap();
    out.into_inner()
}

fn message(bytes: &[u8]) -> String {
    normalize(bytes).err().map(|e| e.1).unwrap_or_default()
}

#[test]
fn logo_size_limits_are_exactly_two_mebibytes() {
    // Exactly 2 MiB passes the size gate and fails as an image; one byte more fails the size gate.
    assert_eq!(message(&vec![0; 2 * 1024 * 1024]), "Invalid company image");
    assert_eq!(
        message(&vec![0; 2 * 1024 * 1024 + 1]),
        "Logo must contain 1 byte..2 MiB"
    );
    assert_eq!(
        message(&vec![0; 1024 * 1024 + 512 * 1024]),
        "Invalid company image"
    );
    assert_eq!(message(&[]), "Logo must contain 1 byte..2 MiB");
}

#[test]
fn a_large_logo_decodes_within_the_allocation_limit_and_the_output_bound() {
    // 700 x 700 noise: about 1.4 MiB of PNG output, under 2 MiB, decoded in about 1.4 MiB of memory.
    let medium = noise(700, ImageFormat::Png);
    assert!(medium.len() < 2 * 1024 * 1024);
    let (out, w, h) = normalize(&medium).unwrap();
    assert_eq!((w, h), (700, 700));
    assert!(out.len() > 1024 * 1024 + 2);
    // 1000 x 1000 noise as JPEG fits the input limit, but its PNG re-encoding exceeds 2 MiB.
    let jpeg = noise(1000, ImageFormat::Jpeg);
    assert!(jpeg.len() <= 2 * 1024 * 1024);
    assert_eq!(message(&jpeg), "Normalized logo exceeds 2 MiB");
}

#[test]
fn a_company_subdivision_must_belong_to_its_country() {
    let mut s = settings();
    s.country_definitions = vec![
        serde_json::from_value(json!({"code":"XQ","alpha3":"","numeric":"",
        "continent":"EU","name":{"en":"Custom"},"states":[{"code":"XQ-N","name":{"en":"North"}}]}))
        .unwrap(),
    ];
    let in_country = |state: &str| company(json!({"country":"XQ","countryStateId":state}));
    assert!(validate(&in_country("XQ-N"), &s, false).is_ok());
    assert!(validate(&in_country("XQ-S"), &s, false).is_err());
}

#[test]
fn an_edge_from_an_unknown_state_is_refused_even_when_all_states_stay_reachable() {
    let mut m = extended(1);
    m.transitions.push(edge("ghost", "nowhere", "extra_0"));
    assert!(m.validate().is_err());
}
