//! The priced quote as checkout, promotions and app adjustments rewrite it: line taxes in quote field names, a line's recalculated price and the quote totals.
use crate::pricing::{CalculatedTax, math_round};
use serde::{Deserialize, Serialize};

/// One tax of a quote line, in the quote's field names.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuoteTax {
    pub tax: f64,
    pub tax_rate: f64,
    pub price: f64,
}

impl From<&CalculatedTax> for QuoteTax {
    fn from(t: &CalculatedTax) -> Self {
        Self {
            tax: t.tax,
            tax_rate: t.tax_rate,
            price: t.price,
        }
    }
}

impl From<&QuoteTax> for CalculatedTax {
    fn from(t: &QuoteTax) -> Self {
        Self {
            tax: t.tax,
            tax_rate: t.tax_rate,
            price: t.price,
        }
    }
}

/// The recalculated price of a quote line; merged into its stored price, the other price fields stay.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinePriceUpdate {
    pub total_price: f64,
    pub unit_price: f64,
    pub calculated_taxes: Vec<QuoteTax>,
}

impl LinePriceUpdate {
    /// The line price for a recalculated total over `quantity` units.
    pub fn new(total_price: f64, quantity: f64, decimals: i32, taxes: &[CalculatedTax]) -> Self {
        Self {
            total_price,
            unit_price: math_round(total_price / quantity, decimals),
            calculated_taxes: taxes.iter().map(QuoteTax::from).collect(),
        }
    }
}

/// The quote totals; merged into the quote price, the other price fields stay.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuoteTotals {
    pub position_price: f64,
    pub total_price: f64,
    pub tax: f64,
    pub net_price: f64,
}

impl QuoteTotals {
    /// The totals of rounded position and tax sums: net quotes add the tax, gross quotes contain it.
    pub fn new(position: f64, tax: f64, decimals: i32, net: bool) -> Self {
        Self {
            position_price: position,
            tax,
            net_price: if net {
                position
            } else {
                math_round(position - tax, decimals)
            },
            total_price: if net {
                math_round(position + tax, decimals)
            } else {
                position
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LinePriceUpdate, QuoteTax, QuoteTotals};
    use crate::pricing::CalculatedTax;
    use serde_json::json;

    #[test]
    fn quote_parts_keep_the_stored_field_names_and_rounding() {
        let tax = CalculatedTax {
            tax: 1.6,
            tax_rate: 19.,
            price: 10.,
        };
        let update = LinePriceUpdate::new(10., 3., 2, std::slice::from_ref(&tax));
        assert_eq!(
            json!(update),
            json!({"totalPrice":10.0,"unitPrice":3.33,"calculatedTaxes":[{"tax":1.6,"taxRate":19.0,"price":10.0}]})
        );
        assert_eq!(CalculatedTax::from(&QuoteTax::from(&tax)), tax);
        assert_eq!(
            json!(QuoteTotals::new(10., 1.6, 2, false)),
            json!({"positionPrice":10.0,"totalPrice":10.0,"tax":1.6,"netPrice":8.4})
        );
        assert_eq!(
            json!(QuoteTotals::new(10., 1.9, 2, true)),
            json!({"positionPrice":10.0,"totalPrice":11.9,"tax":1.9,"netPrice":10.0})
        );
    }
}
