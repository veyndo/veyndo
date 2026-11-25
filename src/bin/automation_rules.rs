//! JSON batch transport for comparisons with original Shopware rule classes; not a production authority endpoint.
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Read};
use veyndo_core::document::{lenient, view};

/// One rule case: the rule class, its configuration and the facts it is evaluated against.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Case {
    #[serde(deserialize_with = "lenient")]
    name: Option<String>,
    config: Value,
    facts: Value,
}
fn main() {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&s).unwrap();
    println!(
        "{}",
        json!(
            cases
                .iter()
                .map(view::<Case>)
                .map(|c| {
                    match veyndo::automation_rules::evaluate(
                        c.name.as_deref().unwrap(),
                        &c.config,
                        &c.facts,
                    ) {
                        Ok(v) => json!(v),
                        Err(e) => json!({"error":e}),
                    }
                })
                .collect::<Vec<_>>()
        )
    );
}
