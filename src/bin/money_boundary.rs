//! Batch transport for comparing the wire-to-minor-unit checkout boundary against original Shopware totals.
use std::io::{self, Read};
use veyndo::money::{Currency, Money};
use veyndo_pricing::decimal::Decimal;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    // Totals arrive as JSON numbers: read exactly, then the one rounding into minor units.
    let totals: Vec<Decimal> = serde_json::from_str(&input)?;
    let output = totals
        .into_iter()
        .map(|total| {
            if total < Decimal::ZERO {
                return Err(io::Error::other("Invalid checkout amount"));
            }
            Money::from_decimal(total, Currency::eur()).map_err(io::Error::other)
        })
        .collect::<Result<Vec<_>, _>>()?;
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
