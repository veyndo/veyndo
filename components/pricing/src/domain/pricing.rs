//! The quantity price calculation in exact decimal arithmetic with whole minor-unit results.
//!
//! Its structure follows Shopware's quantity calculators (6.7.14.2; Copyright (c) 2019
//! shopware AG, MIT; see THIRD_PARTY.md): unit price, per-rule taxes, list, regulation and
//! reference prices. The arithmetic is exact: inputs are decimals or rationals, every
//! intermediate value is an exact [`Ratio`], and each amount is rounded once, half away
//! from zero, to the precision through the proved `rounded_quotient` ([`Ratio::rounded`]).
//! Gross prices and prices with `round_for_net` are then rounded to the cash interval in
//! whole minor units: `rounded_quotient` of the amount by the interval, times the interval
//! by `line_total`. The line total is the proved `line_total` of the rounded unit price and
//! the quantity ([`Minor::times`]). Since
//! 26 November the calculation no longer reproduces the original's binary64 steps, so at a
//! decimal tie the decimal decides: 130.4835 × 670 = 87423.945 rounds to 87423.95.
use crate::decimal::{Decimal, MoneyError, Ratio};
use crate::money::Minor;
use crate::verified_kernel;
use serde::{Deserialize, Serialize};

/// The inputs of one quantity price. Amounts are exact: JSON numbers read as the decimals
/// they carry ([`Decimal::from_wire`]); callers may pass rationals such as a net price.
#[derive(Clone, Debug, Deserialize)]
pub struct PriceInput {
    pub price: Ratio,
    pub quantity: u32,
    pub tax_rate: Decimal,
    #[serde(default = "yes")]
    pub gross: bool,
    #[serde(default = "yes")]
    pub calculated: bool,
    #[serde(default = "decimals")]
    pub decimals: i32,
    #[serde(default = "interval")]
    pub interval: Decimal,
    #[serde(default)]
    pub round_for_net: bool,
    #[serde(default)]
    pub tax_rules: Option<Vec<TaxRule>>,
    #[serde(default)]
    pub list_price: Option<Ratio>,
    #[serde(default)]
    pub regulation_price: Option<Ratio>,
    #[serde(default)]
    pub reference: Option<ReferenceDefinition>,
}
impl Default for PriceInput {
    fn default() -> Self {
        Self {
            price: Ratio::ZERO,
            quantity: 1,
            tax_rate: Decimal::integer(19),
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
/// A tax rate and the percentage of the price it applies to (exact, e.g. a proportional share).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TaxRule {
    pub tax_rate: Decimal,
    pub percentage: Ratio,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReferenceDefinition {
    pub purchase_unit: Decimal,
    pub reference_unit: Decimal,
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
/// The default cash interval: one cent.
fn interval() -> Decimal {
    Decimal::new(1, 2).unwrap_or(Decimal::ZERO)
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

fn hundred() -> Ratio {
    Ratio::integer(100)
}
/// `value / 100`.
fn percent(value: Ratio) -> Result<Ratio, MoneyError> {
    value.checked_div(hundred())
}

/// The calculation's settings: precision, interval and gross/net mode.
struct Rounding {
    decimals: u8,
    scale: u8,
    /// The cash interval in minor units of `scale`, when prices are cash rounded.
    step: Option<u64>,
}
impl Rounding {
    fn new(i: &PriceInput) -> Result<Self, MoneyError> {
        let decimals = u8::try_from(i.decimals)
            .ok()
            .filter(|d| *d <= 18)
            .ok_or("Unsupported price precision")?;
        if i.interval <= Decimal::ZERO {
            return Err("Rounding interval must be positive");
        }
        // Amounts carry the precision's scale, or the interval's when it is finer.
        let interval_scale =
            u8::try_from(i.interval.normalized().scale()).map_err(|_| "Amount overflow")?;
        let scale = if decimals <= 2 {
            decimals.max(interval_scale)
        } else {
            decimals
        };
        // The scale covers the interval's decimals, so the interval is a whole number of units.
        let step = if (i.gross || i.round_for_net) && decimals <= 2 {
            let units = i.interval.ratio().rounded(scale)?.minor();
            Some(u64::try_from(units).map_err(|_| "Amount overflow")?)
        } else {
            None
        };
        Ok(Self {
            decimals,
            scale,
            step,
        })
    }
    /// The value rounded once to the precision, half away from zero, at the amount scale.
    fn plain(&self, value: Ratio) -> Result<Minor, MoneyError> {
        value.rounded(self.decimals)?.at_scale(self.scale)
    }
    /// A price: plain rounding, then for gross prices (or when asked) and precisions of up
    /// to two decimals the nearest multiple of the cash interval, half away from zero: the
    /// proved `rounded_quotient` of the minor units by the interval's, times the interval
    /// through the proved `line_total`. Operands outside the rules' range are refused.
    fn price(&self, value: Ratio) -> Result<Minor, MoneyError> {
        let plain = self.plain(value)?;
        let Some(step) = self.step else {
            return Ok(plain);
        };
        let magnitude = plain.minor().unsigned_abs();
        magnitude.checked_add(step / 2).ok_or("Amount overflow")?;
        let steps = verified_kernel::rounded_quotient(magnitude, step);
        steps.checked_mul(step).ok_or("Amount overflow")?;
        let rounded = i64::try_from(verified_kernel::line_total(steps, step))
            .map_err(|_| "Amount overflow")?;
        Ok(Minor::new(
            if plain.minor() < 0 { -rounded } else { rounded },
            self.scale,
        ))
    }
}

/// The rules of a calculation: duplicate rates replace the earlier rule in its original slot.
fn merged_rules(i: &PriceInput) -> Vec<(Decimal, Ratio)> {
    let mut rules: Vec<(Decimal, Ratio)> = vec![];
    let single = [TaxRule {
        tax_rate: i.tax_rate,
        percentage: hundred(),
    }];
    for r in i.tax_rules.as_deref().unwrap_or(&single) {
        if let Some(old) = rules.iter_mut().find(|old| old.0 == r.tax_rate) {
            *old = (r.tax_rate, r.percentage);
        } else {
            rules.push((r.tax_rate, r.percentage));
        }
    }
    rules
}

/// The price of `quantity` units: the rounded unit price, its proved line total, one tax per
/// rule rounded once from the exact line amount, and the list, regulation and reference prices.
pub fn calculate(i: &PriceInput) -> Result<Price, MoneyError> {
    let rules = merged_rules(i);
    let rounding = Rounding::new(i)?;
    // A net price as gross: the price plus each rule's tax on its share.
    let gross = |price: Ratio| -> Result<Ratio, MoneyError> {
        rules.iter().try_fold(price, |total, (rate, share)| {
            total.checked_add(
                percent(price)?
                    .checked_mul(*share)?
                    .checked_mul(percent(rate.ratio())?)?,
            )
        })
    };
    let uncalculated = i.gross && !i.calculated;
    let input = |price: Ratio| {
        if uncalculated {
            gross(price)
        } else {
            Ok(price)
        }
    };
    let unit = rounding.price(input(i.price)?)?;
    let total_price = unit.times(i.quantity.into())?;
    let line = total_price.ratio();
    let mut tax = Minor::zero(rounding.scale);
    let calculated_taxes = rules
        .iter()
        .map(|(rate, share)| {
            // The rule's share of the line, and the tax it contains (gross) or adds (net).
            let allocated = percent(line)?.checked_mul(*share)?;
            let rate_part = if i.gross {
                rate.ratio()
                    .checked_div(hundred().checked_add(rate.ratio())?)?
            } else {
                percent(rate.ratio())?
            };
            let line_tax = rounding.plain(allocated.checked_mul(rate_part)?)?;
            tax = tax.checked_add(line_tax)?;
            Ok(CalculatedTax {
                tax: line_tax,
                tax_rate: *rate,
                price: rounding.plain(allocated)?,
            })
        })
        .collect::<Result<Vec<_>, MoneyError>>()?;
    let ancillary = |value: Option<Ratio>| -> Result<Option<Minor>, MoneyError> {
        value
            .filter(|p| !p.is_zero())
            .map(|p| rounding.price(input(p)?))
            .transpose()
    };
    let list_price = ancillary(i.list_price)?
        .filter(|p| p.minor() > 0)
        .map(|p| -> Result<ListPrice, MoneyError> {
            Ok(ListPrice {
                price: p,
                discount: unit.checked_sub(p)?,
                percentage: hundred()
                    .checked_sub(
                        unit.ratio()
                            .checked_div(p.ratio())?
                            .checked_mul(hundred())?,
                    )?
                    .rounded(2)?
                    .decimal(),
            })
        })
        .transpose()?;
    let regulation_price = ancillary(i.regulation_price)?;
    let reference_price = i
        .reference
        .as_ref()
        .filter(|r| r.purchase_unit > Decimal::ZERO && r.reference_unit > Decimal::ZERO)
        .map(|r| -> Result<ReferencePrice, MoneyError> {
            Ok(ReferencePrice {
                price: rounding.plain(
                    unit.ratio()
                        .checked_div(r.purchase_unit.ratio())?
                        .checked_mul(r.reference_unit.ratio())?,
                )?,
                purchase_unit: r.purchase_unit,
                reference_unit: r.reference_unit,
                unit_name: r.unit_name.clone(),
            })
        })
        .transpose()?;
    Ok(Price {
        unit_price: unit,
        total_price,
        tax,
        quantity: i.quantity,
        calculated_taxes,
        list_price,
        regulation_price,
        reference_price,
    })
}

/// Each rate's share of `total` in percent, or an even split for a zero total (the rules
/// of PercentageTaxRuleBuilder::buildCollectionRules, exact). `taxes` are (rate, price)
/// pairs of a collection already merged by tax rate.
pub fn proportional_tax_rules(
    taxes: &[(Decimal, Ratio)],
    total: Ratio,
) -> Result<Vec<TaxRule>, MoneyError> {
    let count = i128::try_from(taxes.len()).map_err(|_| "Amount overflow")?;
    taxes
        .iter()
        .map(|(tax_rate, price)| {
            let percentage = if total.is_zero() {
                hundred().checked_div(Ratio::integer(count))?
            } else {
                price.checked_div(total)?.checked_mul(hundred())?
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
    fn r(text: &str) -> Ratio {
        d(text).ratio()
    }
    fn m(text: &str, scale: u8) -> Minor {
        Minor::from_decimal(d(text), scale).unwrap()
    }
    fn price(input: PriceInput) -> Price {
        calculate(&input).unwrap()
    }

    #[test]
    fn cash_rounding_applies_the_interval_up_to_two_decimals() {
        for (value, decimals, interval, expected) in [
            ("1.02", 2, "0.05", "1"),
            ("1.03", 2, "0.05", "1.05"),
            ("1.025", 2, "0.05", "1.05"),
            ("-1.03", 2, "0.05", "-1.05"),
            ("1.005", 2, "0.01", "1.01"),
            ("1.2346", 3, "0.05", "1.235"),
            ("7.4", 0, "1", "7"),
            ("7.5", 0, "1", "8"),
        ] {
            let p = price(PriceInput {
                price: r(value),
                decimals,
                interval: d(interval),
                ..PriceInput::default()
            });
            assert_eq!(p.unit_price.decimal(), d(expected), "{value}");
        }
    }
    #[test]
    fn proportional_rules_split_by_price_or_evenly_for_a_zero_total() {
        assert!(proportional_tax_rules(&[], r("100")).unwrap().is_empty());
        let taxes = [(d("19"), r("75")), (d("7"), r("25"))];
        let rules = proportional_tax_rules(&taxes, r("100")).unwrap();
        assert_eq!(
            rules
                .iter()
                .map(|t| (t.tax_rate, t.percentage))
                .collect::<Vec<_>>(),
            [(d("19"), r("75")), (d("7"), r("25"))]
        );
        let even = proportional_tax_rules(&taxes, Ratio::ZERO).unwrap();
        assert_eq!(
            even.iter().map(|t| t.percentage).collect::<Vec<_>>(),
            [r("50"), r("50")]
        );
        // A third stays a third: no binary or fourteen-digit share.
        let thirds =
            proportional_tax_rules(&[(d("19"), r("1")), (d("7"), r("2"))], r("3")).unwrap();
        assert_eq!(thirds[0].percentage, Ratio::new(100, 3).unwrap());
    }
    /// The ties that changed on 26 November: the binary64 steps had decided them by binary error.
    #[test]
    fn decimal_ties_round_half_away_from_zero() {
        // 130.4835 × 670 = 87423.945 exactly (binary64: 87423.94499999999 → 87423.94).
        let product = r("130.4835").checked_mul(Ratio::integer(670)).unwrap();
        assert_eq!(product.rounded(2).unwrap(), Minor::new(8_742_395, 2));
        // A net tax at a tie: 651 × 73 × 5 % = 2376.15 → 2376.2 (binary64: 2376.1).
        let p = price(PriceInput {
            price: r("651"),
            quantity: 73,
            tax_rate: d("5"),
            gross: false,
            decimals: 1,
            interval: d("0.5"),
            round_for_net: true,
            ..PriceInput::default()
        });
        assert_eq!(p.tax, m("2376.2", 1));
        // A gross unit price at a tie: 100.205 × 1.07 = 107.21935 → 107.2194 (binary64: 107.2193).
        let p = price(PriceInput {
            price: r("100.205"),
            quantity: 6,
            tax_rate: d("7"),
            calculated: false,
            decimals: 4,
            ..PriceInput::default()
        });
        assert_eq!(p.unit_price, m("107.2194", 4));
        assert_eq!(p.total_price, m("643.3164", 4));
        assert_eq!(r("1.005").rounded(2).unwrap().minor(), 101);
        assert_eq!(r("-1.005").rounded(2).unwrap().minor(), -101);
        assert_eq!(r("1.004999").rounded(2).unwrap().minor(), 100);
    }
    /// Negative prices round half away from zero, symmetrically. -725.879 × 69 at 20 % gross
    /// contains -8347.6085 tax exactly: -8347.609 (binary64 had -8347.608).
    #[test]
    fn negative_prices_round_symmetrically() {
        for (unit, quantity, tax) in [
            ("-853.999", 93, "-13236.985"),
            ("-725.879", 69, "-8347.609"),
        ] {
            let p = price(PriceInput {
                price: r(unit),
                quantity,
                tax_rate: d("20"),
                decimals: 3,
                ..PriceInput::default()
            });
            assert_eq!(p.tax.decimal(), d(tax));
        }
    }
    #[test]
    fn gross_unit_before_total() {
        let p = price(PriceInput {
            price: r("19.995"),
            quantity: 3,
            ..PriceInput::default()
        });
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
                    interval: d(interval),
                    ..PriceInput::default()
                })
                .is_err()
            );
        }
    }
    #[test]
    fn wire_inputs_read_as_their_decimals() {
        let input: PriceInput = serde_json::from_value(serde_json::json!({
            "price": 130.4835, "quantity": 670, "tax_rate": 0, "gross": false, "decimals": 4,
            "tax_rules": [{"tax_rate": 0, "percentage": 100}]
        }))
        .unwrap();
        assert_eq!(input.price, r("130.4835"));
        assert_eq!(price(input).total_price, m("87423.945", 4));
    }
}
