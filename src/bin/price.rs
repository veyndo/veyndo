//! Batch price fixture transport for the original-PHP differential comparator.
use std::io::{self, Read};
use veyndo::pricing::{PriceInput, calculate};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let cases: Vec<PriceInput> = serde_json::from_str(&input)?;
    let prices = cases
        .iter()
        .map(calculate)
        .collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)?;
    println!("{}", serde_json::to_string(&prices)?);
    Ok(())
}
