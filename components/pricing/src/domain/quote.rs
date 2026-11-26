//! The priced quote as checkout, promotions and app adjustments rewrite it: line taxes in quote field names, a line's recalculated price and the quote totals.
use crate::decimal::{Decimal, MoneyError, Ratio};
use crate::money::{Minor, unsigned_sum};
use crate::pricing::CalculatedTax;
use crate::verified_kernel;
use serde::{Deserialize, Serialize};

/// One tax of a quote line, in the quote's field names; stored amounts are read exactly.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuoteTax {
    pub tax: Decimal,
    pub tax_rate: Decimal,
    pub price: Decimal,
}

impl From<&CalculatedTax> for QuoteTax {
    fn from(t: &CalculatedTax) -> Self {
        Self {
            tax: t.tax.decimal(),
            tax_rate: t.tax_rate,
            price: t.price.decimal(),
        }
    }
}

impl QuoteTax {
    /// The stored tax as a calculated tax in minor units of `scale`.
    pub fn at_scale(&self, scale: u8) -> Result<CalculatedTax, MoneyError> {
        Ok(CalculatedTax {
            tax: Minor::from_decimal(self.tax, scale)?,
            tax_rate: self.tax_rate,
            price: Minor::from_decimal(self.price, scale)?,
        })
    }
}

/// The recalculated price of a quote line; merged into its stored price, the other price fields stay.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinePriceUpdate {
    pub total_price: Minor,
    pub unit_price: Minor,
    pub calculated_taxes: Vec<QuoteTax>,
}

impl LinePriceUpdate {
    /// The line price for a recalculated total over `quantity` units (at least one); the unit
    /// price is `total / quantity` rounded once, half away from zero.
    pub fn new(
        total_price: Minor,
        quantity: u64,
        taxes: &[CalculatedTax],
    ) -> Result<Self, MoneyError> {
        let units = Ratio::integer(quantity.max(1).into());
        Ok(Self {
            total_price,
            unit_price: total_price
                .ratio()
                .checked_div(units)?
                .rounded(total_price.scale())?,
            calculated_taxes: taxes.iter().map(QuoteTax::from).collect(),
        })
    }
}

/// The quote totals; merged into the quote price, the other price fields stay.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuoteTotals {
    pub position_price: Minor,
    pub total_price: Minor,
    pub tax: Minor,
    pub net_price: Minor,
}

impl QuoteTotals {
    /// The totals of the position and tax sums: net quotes add the tax (`amount_sum`), gross
    /// quotes contain it (`net_amount`). Negative amounts are refused, sums never wrap.
    pub fn new(position: Minor, tax: Minor, net: bool) -> Result<Self, MoneyError> {
        let scale = position.scale();
        let (goods, taxes) = (position.unsigned()?, tax.at_scale(scale)?.unsigned()?);
        let (total, net_price) = if net {
            (unsigned_sum([goods, taxes])?, goods)
        } else {
            (goods, verified_kernel::net_amount(goods, taxes))
        };
        Ok(Self {
            position_price: position,
            tax,
            net_price: Minor::from_unsigned(net_price, scale)?,
            total_price: Minor::from_unsigned(total, scale)?,
        })
    }
    /// The totals of priced lines: the goods and tax sums of their stored amounts.
    pub fn of_lines(
        lines: &[Minor],
        taxes: &[Minor],
        scale: u8,
        net: bool,
    ) -> Result<Self, MoneyError> {
        let sum = |values: &[Minor]| -> Result<Minor, MoneyError> {
            let minor = unsigned_sum(
                values
                    .iter()
                    .map(|v| v.at_scale(scale)?.unsigned())
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            Minor::from_unsigned(minor, scale)
        };
        Self::new(sum(lines)?, sum(taxes)?, net)
    }
}

#[cfg(test)]
mod tests {
    use super::{LinePriceUpdate, QuoteTax, QuoteTotals};
    use crate::decimal::Decimal;
    use crate::money::Minor;
    use crate::pricing::CalculatedTax;
    use serde_json::json;

    #[test]
    fn quote_parts_keep_the_stored_field_names_and_rounding() {
        let tax = CalculatedTax {
            tax: Minor::new(160, 2),
            tax_rate: Decimal::integer(19),
            price: Minor::new(1000, 2),
        };
        let update =
            LinePriceUpdate::new(Minor::new(1000, 2), 3, std::slice::from_ref(&tax)).unwrap();
        assert_eq!(
            json!(update),
            json!({"totalPrice":10.0,"unitPrice":3.33,"calculatedTaxes":[{"tax":1.6,"taxRate":19.0,"price":10.0}]})
        );
        assert_eq!(QuoteTax::from(&tax).at_scale(2).unwrap(), tax);
        assert_eq!(
            json!(QuoteTotals::new(Minor::new(1000, 2), Minor::new(160, 2), false).unwrap()),
            json!({"positionPrice":10.0,"totalPrice":10.0,"tax":1.6,"netPrice":8.4})
        );
        assert_eq!(
            json!(QuoteTotals::new(Minor::new(1000, 2), Minor::new(190, 2), true).unwrap()),
            json!({"positionPrice":10.0,"totalPrice":11.9,"tax":1.9,"netPrice":10.0})
        );
        assert!(QuoteTotals::new(Minor::new(-1, 2), Minor::zero(2), true).is_err());
        assert!(QuoteTotals::new(Minor::new(i64::MAX, 2), Minor::new(1, 2), true).is_err());
        assert_eq!(
            QuoteTotals::of_lines(
                &[Minor::new(250, 2), Minor::new(750, 2)],
                &[Minor::new(160, 2)],
                2,
                false
            )
            .unwrap(),
            QuoteTotals::new(Minor::new(1000, 2), Minor::new(160, 2), false).unwrap()
        );
    }
}
