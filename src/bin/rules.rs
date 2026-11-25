//! Batch original-PHP numeric-rule comparison transport.
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Read};
use veyndo_core::document::{lenient, view};

/// One rule comparison case: the operator, the comparison kind and both operands as sent.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Case {
    #[serde(deserialize_with = "lenient")]
    operator: Option<String>,
    #[serde(deserialize_with = "lenient")]
    kind: Option<String>,
    item: Value,
    rule: Value,
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&input).unwrap();
    let out = cases
        .iter()
        .map(view::<Case>)
        .map(|v| {
            let op = v.operator.as_deref().unwrap();
            let strings = |value: &Value| {
                value.as_array().map(|a| {
                    a.iter()
                        .map(|x| x.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
            };
            let result = match v.kind.as_deref().unwrap_or("numeric") {
                "string" => veyndo::rule_comparison::string(
                    v.item.as_str(),
                    v.rule.as_str().unwrap_or(""),
                    op,
                ),
                "stringArray" => veyndo::rule_comparison::string_array(
                    v.item.as_str(),
                    &v.rule
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().into())
                        .collect::<Vec<_>>(),
                    op,
                ),
                "uuids" => veyndo::rule_comparison::uuids(
                    strings(&v.item).as_deref(),
                    strings(&v.rule).as_deref(),
                    op,
                ),
                _ => veyndo::rule_comparison::numeric(v.item.as_f64(), v.rule.as_f64(), op),
            };
            match result {
                Ok(b) => json!(b),
                Err(_) => json!({"error":true}),
            }
        })
        .collect::<Vec<_>>();
    println!("{}", json!(out));
}
