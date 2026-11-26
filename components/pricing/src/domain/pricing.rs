//! Behavioral port of Shopware 6.7.14.2 quantity calculators with whole minor-unit results.
//! Original: Copyright (c) 2019 shopware AG, MIT; see THIRD_PARTY.md.
//!
//! The original computes in binary64 and keeps intermediate values at fourteen significant
//! digits (`FloatComparator::cast`). Its results at decimal ties depend on that binary
//! arithmetic, so this port performs the same steps on [`Binary`], an exact integer
//! emulation of binary64 (no floating-point type). Every result leaves the calculation as
//! a [`Minor`] amount in whole minor units: the decimal the original writes, exactly. The
//! line total is the proved `line_total` of the unit price and the quantity.
use crate::binary::Binary;
use crate::decimal::{Decimal, MoneyError};
use crate::money::Minor;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PriceInput {
    pub price: Binary,
    pub quantity: u32,
    pub tax_rate: Binary,
    #[serde(default = "yes")]
    pub gross: bool,
    #[serde(default = "yes")]
    pub calculated: bool,
    #[serde(default = "decimals")]
    pub decimals: i32,
    #[serde(default = "interval")]
    pub interval: Binary,
    #[serde(default)]
    pub round_for_net: bool,
    #[serde(default)]
    pub tax_rules: Option<Vec<TaxRule>>,
    #[serde(default)]
    pub list_price: Option<Binary>,
    #[serde(default)]
    pub regulation_price: Option<Binary>,
    #[serde(default)]
    pub reference: Option<ReferenceDefinition>,
}
impl Default for PriceInput {
    fn default() -> Self {
        Self {
            price: Binary::ZERO,
            quantity: 1,
            tax_rate: Binary::integer(19),
            gross: true,
            calculated: true,
            decimals: 2,
            interval: interval(),
            round_for_net: false,
            tax_rules: None,
            list_price: None,
            regulation_price: None,
            reference: None,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaxRule {
    pub tax_rate: Binary,
    pub percentage: Binary,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReferenceDefinition {
    pub purchase_unit: Binary,
    pub reference_unit: Binary,
    pub unit_name: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CalculatedTax {
    pub tax: Minor,
    pub tax_rate: Decimal,
    pub price: Minor,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ListPrice {
    pub price: Minor,
    pub discount: Minor,
    pub percentage: Decimal,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReferencePrice {
    pub price: Minor,
    pub purchase_unit: Decimal,
    pub reference_unit: Decimal,
    pub unit_name: String,
}
fn yes() -> bool {
    true
}
fn decimals() -> i32 {
    2
}
/// 0.01 as the original's literal reads it.
fn interval() -> Binary {
    Binary::from_decimal(Decimal::new(1, 2).unwrap_or(Decimal::ZERO)).unwrap_or(Binary::ZERO)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Price {
    pub unit_price: Minor,
    pub total_price: Minor,
    pub tax: Minor,
    pub quantity: u32,
    pub calculated_taxes: Vec<CalculatedTax>,
    pub list_price: Option<ListPrice>,
    pub regulation_price: Option<Minor>,
    pub reference_price: Option<ReferencePrice>,
}

/// The original `round(value, decimals)` as an amount in minor units of `decimals`.
pub fn math_round(value: Binary, decimals: u8) -> Result<Minor, MoneyError> {
    value.round(decimals)?.minor(decimals)
}

/// `value / 100` as the original divides.
fn percent(value: Binary) -> Result<Binary, MoneyError> {
    value.checked_div(Binary::integer(100))
}
/// The sum of the original's tax amounts: one precision step, and zero below 1e-8.
fn sum(values: impl IntoIterator<Item = Binary>) -> Result<Binary, MoneyError> {
    let total = values
        .into_iter()
        .try_fold(Binary::ZERO, Binary::checked_add)?
        .cast()?;
    let threshold = Binary::from_decimal(Decimal::new(1, 8)?)?;
    Ok(if total.abs() < threshold {
        Binary::ZERO
    } else {
        total
    })
}

/// The calculation's settings: precision, interval and gross/net mode.
struct Rounding {
    decimals: u8,
    scale: u8,
    interval: Binary,
    cash: bool,
}
impl Rounding {
    fn new(i: &PriceInput) -> Result<Self, MoneyError> {
        let decimals = u8::try_from(i.decimals)
            .ok()
            .filter(|d| *d <= 18)
            .ok_or("Unsupported price precision")?;
        if i.interval <= Binary::ZERO {
            return Err("Rounding interval must be positive");
        }
        // Amounts carry the precision's scale, or the interval's when it is finer.
        let interval_scale =
            u8::try_from(i.interval.shortest()?.scale()).map_err(|_| "Amount overflow")?;
        let scale = if decimals <= 2 {
            decimals.max(interval_scale)
        } else {
            decimals
        };
        Ok(Self {
            decimals,
            scale,
            interval: i.interval,
            cash: i.gross || i.round_for_net,
        })
    }
    /// The original `round` closure: cash rounding for gross prices or when asked, else plain.
    fn round(&self, value: Binary) -> Result<Binary, MoneyError> {
        if self.cash {
            value.cash_round(self.decimals, self.interval)
        } else {
            value.round(self.decimals)
        }
    }
    /// What the original writes for a value: its fourteen-digit decimal, in minor units.
    fn minor(&self, value: Binary) -> Result<Minor, MoneyError> {
        value.minor(self.scale)
    }
}

/// The rules of a calculation: duplicate rates replace the earlier rule in its original slot.
fn merged_rules(i: &PriceInput) -> Result<Vec<(Binary, Binary)>, MoneyError> {
    let mut rules: Vec<(Binary, Binary)> = vec![];
    let single = [TaxRule {
        tax_rate: i.tax_rate,
        percentage: Binary::integer(100),
    }];
    for r in i.tax_rules.as_deref().unwrap_or(&single) {
        let rule = (r.tax_rate.cast()?, r.percentage.cast()?);
        if let Some(old) = rules.iter_mut().find(|old| old.0 == rule.0) {
            *old = rule;
        } else {
            rules.push(rule);
        }
    }
    Ok(rules)
}

pub fn calculate(i: &PriceInput) -> Result<Price, MoneyError> {
    let rules = merged_rules(i)?;
    let rounding = Rounding::new(i)?;
    let quantity = Binary::integer(i.quantity.into());
    let hundred = Binary::integer(100);
    let net_tax = |price: Binary, (rate, share): &(Binary, Binary)| {
        percent(price)?
            .checked_mul(*share)?
            .checked_mul(percent(*rate)?)?
            .cast()
    };
    let gross = |price: Binary| -> Result<Binary, MoneyError> {
        price.checked_add(sum(rules
            .iter()
            .map(|r| net_tax(price, r))
            .collect::<Result<Vec<_>, _>>()?)?)
    };
    let uncalculated = i.gross && !i.calculated;
    let input = i.price.cast()?;
    let unit = rounding.round(if uncalculated { gross(input)? } else { input })?;
    let mut tax_amounts = vec![];
    let calculated_taxes = rules
        .iter()
        .map(|(rate, share)| {
            let allocated = percent(unit)?.checked_mul(*share)?;
            let tax = if i.gross {
                allocated
                    .checked_div(percent(hundred.checked_add(*rate)?)?)?
                    .checked_mul(percent(*rate)?)?
            } else {
                allocated.checked_mul(percent(*rate)?)?
            }
            .cast()?;
            let line_tax = tax
                .checked_mul(quantity)?
                .round(rounding.decimals)?
                .cast()?;
            tax_amounts.push(line_tax);
            Ok(CalculatedTax {
                tax: rounding.minor(line_tax)?,
                tax_rate: rate.decimal()?,
                price: rounding.minor(
                    allocated
                        .cast()?
                        .checked_mul(quantity)?
                        .round(rounding.decimals)?,
                )?,
            })
        })
        .collect::<Result<Vec<_>, MoneyError>>()?;
    let ancillary = |value: Option<Binary>| -> Result<Option<Binary>, MoneyError> {
        value
            .filter(|p| !p.is_zero())
            .map(|p| {
                let p = p.cast()?;
                rounding.round(if uncalculated { gross(p)? } else { p })
            })
            .transpose()
    };
    let list_price = ancillary(i.list_price)?
        .filter(|p| *p > Binary::ZERO)
        .map(|p| -> Result<ListPrice, MoneyError> {
            Ok(ListPrice {
                price: rounding.minor(p)?,
                discount: rounding.minor(p.checked_sub(unit)?.negated())?,
                percentage: hundred
                    .checked_sub(unit.checked_div(p)?.checked_mul(hundred)?)?
                    .round(2)?
                    .decimal()?,
            })
        })
        .transpose()?;
    let regulation_price = ancillary(i.regulation_price)?
        .map(|p| rounding.minor(p))
        .transpose()?;
    let reference_price = i
        .reference
        .as_ref()
        .filter(|r| r.purchase_unit > Binary::ZERO && r.reference_unit > Binary::ZERO)
        .map(|r| -> Result<ReferencePrice, MoneyError> {
            let purchase = r.purchase_unit.cast()?;
            let reference = r.reference_unit.cast()?;
            Ok(ReferencePrice {
                price: rounding.minor(
                    unit.checked_div(purchase)?
                        .checked_mul(reference)?
                        .round(rounding.decimals)?,
                )?,
                purchase_unit: purchase.decimal()?,
                reference_unit: reference.decimal()?,
                unit_name: r.unit_name.clone(),
            })
        })
        .transpose()?;
    let unit_price = rounding.minor(unit)?;
    Ok(Price {
        unit_price,
        total_price: line_total(unit_price, i.quantity)?,
        tax: rounding.minor(sum(tax_amounts)?)?,
        quantity: i.quantity,
        calculated_taxes,
        list_price,
        regulation_price,
        reference_price,
    })
}

/// The largest line total whose binary64 product the original rounds back onto the minor
/// unit grid: below 2^53 minor units a product of a grid amount and a whole quantity is
/// within a fraction of a unit of the grid, so `round(unit * quantity)` is the exact product.
const EXACT_PRODUCT: u64 = 1 << 53;

/// The line total: the proved product of the unit price (already on the rounding interval's
/// grid) and the quantity, written as the original writes it (its binary value's fourteen
/// significant digits; they differ from the exact product only beyond fourteen digits).
fn line_total(unit: Minor, quantity: u32) -> Result<Minor, MoneyError> {
    let total = unit.times(quantity.into())?;
    if total.minor().unsigned_abs() >= EXACT_PRODUCT {
        return Err("Amount overflow");
    }
    Binary::from_decimal(total.decimal())?.minor(total.scale())
}

/// Port of PercentageTaxRuleBuilder::buildCollectionRules (6.7.14.2): each rate's share of
/// `total` in percent, or an even split for a zero total. `taxes` are (rate, price) pairs of a
/// collection already merged by tax rate, as in CalculatedTaxCollection.
pub fn proportional_tax_rules(
    taxes: &[(Binary, Binary)],
    total: Binary,
) -> Result<Vec<TaxRule>, MoneyError> {
    let count = i64::try_from(taxes.len()).map_err(|_| "Amount overflow")?;
    let hundred = Binary::integer(100);
    taxes
        .iter()
        .map(|(tax_rate, price)| {
            let percentage = if total.is_zero() {
                hundred.checked_div(Binary::integer(count))?
            } else {
                price.checked_div(total)?.checked_mul(hundred)?
            };
            Ok(TaxRule {
                tax_rate: *tax_rate,
                percentage,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        Decimal::parse(text).unwrap()
    }
    fn b(text: &str) -> Binary {
        Binary::from_decimal(d(text)).unwrap()
    }
    fn m(text: &str, scale: u8) -> Minor {
        Minor::from_decimal(d(text), scale).unwrap()
    }

    #[test]
    fn cash_rounding_applies_the_interval_up_to_two_decimals() {
        for (value, decimals, interval, expected) in [
            ("1.02", 2, "0.05", "1"),
            ("1.03", 2, "0.05", "1.05"),
            ("-1.03", 2, "0.05", "-1.05"),
            ("1.005", 2, "0.01", "1.01"),
            ("1.2346", 3, "0.05", "1.235"),
            ("7.4", 0, "1", "7"),
        ] {
            assert_eq!(
                b(value)
                    .cash_round(decimals, b(interval))
                    .unwrap()
                    .decimal()
                    .unwrap(),
                d(expected)
            );
        }
    }
    #[test]
    fn proportional_rules_split_by_price_or_evenly_for_a_zero_total() {
        assert!(proportional_tax_rules(&[], b("100")).unwrap().is_empty());
        let taxes = [(b("19"), b("75")), (b("7"), b("25"))];
        let rules = proportional_tax_rules(&taxes, b("100")).unwrap();
        assert_eq!(
            rules
                .iter()
                .map(|r| (r.tax_rate, r.percentage))
                .collect::<Vec<_>>(),
            [(b("19"), b("75")), (b("7"), b("25"))]
        );
        let even = proportional_tax_rules(&taxes, Binary::ZERO).unwrap();
        assert_eq!(
            even.iter().map(|r| r.percentage).collect::<Vec<_>>(),
            [b("50"), b("50")]
        );
    }
    #[test]
    fn decimal_ties() {
        assert_eq!(math_round(b("1.005"), 2).unwrap().minor(), 101);
        assert_eq!(math_round(b("-1.005"), 2).unwrap().minor(), -101);
        assert_eq!(math_round(b("1.004999"), 2).unwrap().minor(), 100);
        // The binary product decides a decimal tie, as in the original.
        assert_eq!(
            math_round(b("130.4835").checked_mul(Binary::integer(670)).unwrap(), 2)
                .unwrap()
                .minor(),
            8_742_394
        );
    }
    #[test]
    fn shopware_negative_tax_cast_regression() {
        for (price, quantity, tax) in [
            ("-853.999", 93, "-13236.985"),
            ("-725.879", 69, "-8347.608"),
        ] {
            let p = calculate(&PriceInput {
                price: b(price),
                quantity,
                tax_rate: b("20"),
                decimals: 3,
                ..PriceInput::default()
            })
            .unwrap();
            assert_eq!(p.tax.decimal(), d(tax));
        }
    }
    #[test]
    fn gross_unit_before_total() {
        let p = calculate(&PriceInput {
            price: b("19.995"),
            quantity: 3,
            ..PriceInput::default()
        })
        .unwrap();
        assert_eq!(p.total_price, m("60", 2));
        assert_eq!(p.tax, m("9.58", 2));
        assert_eq!(
            serde_json::to_value(&p).unwrap()["total_price"],
            serde_json::json!(60.0)
        );
    }
    #[test]
    fn unsupported_precision_and_intervals_are_refused() {
        for (decimals, interval) in [(19, "0.01"), (-1, "0.01"), (2, "0")] {
            assert!(
                calculate(&PriceInput {
                    decimals,
                    interval: b(interval),
                    ..PriceInput::default()
                })
                .is_err()
            );
        }
    }
}
