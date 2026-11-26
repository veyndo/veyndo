//! Batch proportional-tax fixture transport for the original-PHP comparator.
use serde::Deserialize;
use std::io::{self, Read};
use veyndo::pricing::proportional_tax_rules;
use veyndo_pricing::binary::Binary;
/// One calculated tax of the fixture, as JSON numbers.
#[derive(Deserialize)]
struct Tax {
    #[allow(dead_code)]
    tax: Binary,
    tax_rate: Binary,
    price: Binary,
}
#[derive(Deserialize)]
struct Case {
    taxes: Vec<Tax>,
    total: Binary,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let cases: Vec<Case> = serde_json::from_str(&input)?;
    let out = cases
        .iter()
        .map(|c| {
            let shares = c
                .taxes
                .iter()
                .map(|t| (t.tax_rate, t.price))
                .collect::<Vec<_>>();
            proportional_tax_rules(&shares, c.total)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)?;
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
