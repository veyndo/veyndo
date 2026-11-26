//! Exact signed minor-unit amounts with explicit currency scale: `Money` with its currency code, `Minor` on the calculation path.
use crate::decimal::{Decimal, MoneyError, Ratio};
use crate::verified_kernel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CurrencyWire")]
pub struct Currency {
    code: String,
    scale: u8,
}
#[derive(Deserialize)]
struct CurrencyWire {
    code: String,
    scale: u8,
}
impl TryFrom<CurrencyWire> for Currency {
    type Error = &'static str;
    fn try_from(v: CurrencyWire) -> Result<Self, Self::Error> {
        Self::new(&v.code, v.scale)
    }
}
impl Currency {
    pub fn new(code: &str, scale: u8) -> Result<Self, &'static str> {
        if code.len() != 3
            || !code.bytes().all(|b| b.is_ascii_uppercase())
            || !crate::verified_kernel::currency_scale_admissible(u64::from(scale))
        {
            return Err("Invalid currency code or scale");
        }
        Ok(Self {
            code: code.into(),
            scale,
        })
    }
    pub fn eur() -> Self {
        Self {
            code: "EUR".into(),
            scale: 2,
        }
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn scale(&self) -> u8 {
        self.scale
    }
    fn factor(&self) -> i64 {
        10_i64.pow(self.scale.into())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    minor: i64,
    currency: Currency,
}
impl Money {
    pub fn new(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }
    pub fn minor(&self) -> i64 {
        self.minor
    }
    pub fn currency(&self) -> &Currency {
        &self.currency
    }
    /// Integer decimal parsing: no exponent, floating-point intermediary, truncation or hidden rounding.
    pub fn parse(value: &str, currency: Currency) -> Result<Self, &'static str> {
        let (negative, value) = value
            .strip_prefix('-')
            .map(|v| (true, v))
            .unwrap_or((false, value));
        let (major, fraction) = value.split_once('.').unwrap_or((value, ""));
        if major.is_empty()
            || !major.bytes().all(|b| b.is_ascii_digit())
            || fraction.len() != currency.scale as usize
            || !fraction.bytes().all(|b| b.is_ascii_digit())
            || currency.scale == 0 && value.contains('.')
        {
            return Err("Amount must use the exact configured currency scale");
        }
        let major = major.parse::<i128>().map_err(|_| "Amount overflow")?;
        let fraction = if fraction.is_empty() {
            0
        } else {
            fraction.parse::<i128>().map_err(|_| "Amount overflow")?
        };
        let absolute = major
            .checked_mul(currency.factor() as i128)
            .and_then(|v| v.checked_add(fraction))
            .ok_or("Amount overflow")?;
        let signed = if negative { -absolute } else { absolute };
        Ok(Self::new(
            i64::try_from(signed).map_err(|_| "Amount overflow")?,
            currency,
        ))
    }
    /// Round a decimal wire amount to the configured minor unit, halfway away from zero.
    /// Decimal digits are authoritative: multiplying a binary float never decides a tie.
    pub fn rounded(value: &str, currency: Currency) -> Result<Self, &'static str> {
        if value.len() > 64 {
            return Err("Amount overflow");
        }
        let (negative, value) = value
            .strip_prefix('-')
            .map(|v| (true, v))
            .unwrap_or((false, value));
        let (major, fraction) = value.split_once('.').unwrap_or((value, ""));
        if major.is_empty()
            || !major.bytes().all(|b| b.is_ascii_digit())
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Decimal amount required");
        }
        let scale = currency.scale as usize;
        let kept = format!(
            "{:0<width$}",
            fraction.chars().take(scale).collect::<String>(),
            width = scale
        );
        let mut absolute = major
            .parse::<i128>()
            .map_err(|_| "Amount overflow")?
            .checked_mul(currency.factor() as i128)
            .and_then(|v| {
                v.checked_add(if kept.is_empty() {
                    0
                } else {
                    kept.parse::<i128>().ok()?
                })
            })
            .ok_or("Amount overflow")?;
        if fraction.as_bytes().get(scale).is_some_and(|d| *d >= b'5') {
            absolute = absolute.checked_add(1).ok_or("Amount overflow")?;
        }
        let signed = if negative { -absolute } else { absolute };
        Ok(Self::new(
            i64::try_from(signed).map_err(|_| "Amount overflow")?,
            currency,
        ))
    }
    pub fn decimal(&self) -> String {
        let absolute = self.minor.unsigned_abs();
        let factor = self.currency.factor() as u64;
        let sign = if self.minor < 0 { "-" } else { "" };
        if self.currency.scale == 0 {
            format!("{sign}{absolute}")
        } else {
            format!(
                "{sign}{}.{:0width$}",
                absolute / factor,
                absolute % factor,
                width = self.currency.scale as usize
            )
        }
    }
    pub fn checked_add(&self, other: &Self) -> Result<Self, &'static str> {
        if self.currency != other.currency {
            return Err("Currency or scale mismatch");
        }
        Ok(Self::new(
            self.minor
                .checked_add(other.minor)
                .ok_or("Amount overflow")?,
            self.currency.clone(),
        ))
    }
    /// An exact decimal amount in this currency's minor units, rounded half away from zero.
    pub fn from_decimal(value: Decimal, currency: Currency) -> Result<Self, MoneyError> {
        let minor = value.ratio().rounded(currency.scale)?;
        Ok(Self::new(minor.minor(), currency))
    }
    /// The amount in whole minor units without its currency code.
    pub fn amount(&self) -> Minor {
        Minor::new(self.minor, self.currency.scale)
    }
}

/// An amount in whole minor units of a currency with `scale` decimal places (EUR cents: 2).
///
/// This is the money type of the calculation path: line totals, discounts, shipping,
/// taxes, net and gross amounts and quote totals. Arithmetic is checked: an overflow
/// or a mismatch of scales is refused, never wrapped. Rounding into it is half away
/// from zero ([`Ratio::rounded`]); JSON writes it as the nearest number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Minor {
    minor: i64,
    scale: u8,
}

impl Minor {
    pub fn new(minor: i64, scale: u8) -> Self {
        Self { minor, scale }
    }
    pub fn zero(scale: u8) -> Self {
        Self { minor: 0, scale }
    }
    pub fn minor(&self) -> i64 {
        self.minor
    }
    pub fn scale(&self) -> u8 {
        self.scale
    }
    pub fn decimal(&self) -> Decimal {
        Decimal::new(self.minor.into(), self.scale.into()).unwrap_or(Decimal::ZERO)
    }
    pub fn ratio(&self) -> Ratio {
        self.decimal().ratio()
    }
    /// An exact amount at `scale`; a value with more decimals is rounded half away from zero.
    pub fn from_decimal(value: Decimal, scale: u8) -> Result<Self, MoneyError> {
        value.ratio().rounded(scale)
    }
    fn same_scale(&self, other: &Self) -> Result<(), MoneyError> {
        if self.scale == other.scale {
            Ok(())
        } else {
            Err("Currency or scale mismatch")
        }
    }
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        self.same_scale(&other)?;
        Ok(Self::new(
            self.minor
                .checked_add(other.minor)
                .ok_or("Amount overflow")?,
            self.scale,
        ))
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.same_scale(&other)?;
        Ok(Self::new(
            self.minor
                .checked_sub(other.minor)
                .ok_or("Amount overflow")?,
            self.scale,
        ))
    }
    /// The amount of `quantity` units: the proved line total on the magnitude, refused
    /// instead of saturating.
    pub fn times(self, quantity: u64) -> Result<Self, MoneyError> {
        let magnitude = self.minor.unsigned_abs();
        if magnitude.checked_mul(quantity).is_none() {
            return Err("Amount overflow");
        }
        let total = i64::try_from(verified_kernel::line_total(magnitude, quantity))
            .map_err(|_| "Amount overflow")?;
        Ok(Self::new(
            if self.minor < 0 { -total } else { total },
            self.scale,
        ))
    }
    /// The same amount at a scale with at least as many decimals.
    pub fn at_scale(self, scale: u8) -> Result<Self, MoneyError> {
        let shift = scale
            .checked_sub(self.scale)
            .ok_or("Currency or scale mismatch")?;
        let factor = 10_i64.checked_pow(shift.into()).ok_or("Amount overflow")?;
        Ok(Self::new(
            self.minor.checked_mul(factor).ok_or("Amount overflow")?,
            scale,
        ))
    }
    /// The amount at `digits` significant digits (the calculators' precision step), same scale.
    pub fn significant(self, digits: u32) -> Result<Self, MoneyError> {
        Self::from_decimal(self.ratio().significant(digits)?, self.scale)
    }
    /// The amount as unsigned minor units for the proved sums; a negative amount is refused.
    pub fn unsigned(self) -> Result<u64, MoneyError> {
        u64::try_from(self.minor).map_err(|_| "Amount must not be negative")
    }
    pub fn from_unsigned(minor: u64, scale: u8) -> Result<Self, MoneyError> {
        Ok(Self::new(
            i64::try_from(minor).map_err(|_| "Amount overflow")?,
            scale,
        ))
    }
    /// The signed sum of amounts at one scale, refused on overflow or mismatched scales.
    pub fn sum(values: impl IntoIterator<Item = Self>, scale: u8) -> Result<Self, MoneyError> {
        values
            .into_iter()
            .try_fold(Self::zero(scale), |total, value| total.checked_add(value))
    }
}

/// JSON writes an amount as the nearest number, through the one wire conversion of [`Decimal`].
impl Serialize for Minor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.decimal().serialize(serializer)
    }
}

/// The sum of unsigned minor amounts through the proved `amount_sum`; refused instead of saturating.
pub fn unsigned_sum(values: impl IntoIterator<Item = u64>) -> Result<u64, MoneyError> {
    values.into_iter().try_fold(0_u64, |total, value| {
        total.checked_add(value).ok_or("Amount overflow")?;
        Ok(verified_kernel::amount_sum(total, value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_currency_keeps_its_code_and_scale() {
        let currency = Currency::new("KWD", 3).unwrap();
        assert_eq!((currency.code(), currency.scale()), ("KWD", 3));
    }
    #[test]
    fn exact_roundtrip_including_signed_extremes_and_explicit_scales() {
        for scale in [0, 2, 3, 6] {
            let currency = Currency::new("XXX", scale).unwrap();
            for value in [i64::MIN, -101, -1, 0, 1, 101, i64::MAX] {
                let m = Money::new(value, currency.clone());
                assert_eq!(Money::parse(&m.decimal(), currency.clone()).unwrap(), m);
            }
        }
        assert_eq!(
            Money::new(1234, Currency::new("KWD", 3).unwrap()).decimal(),
            "1.234"
        );
        assert_eq!(
            Money::new(1234, Currency::new("JPY", 0).unwrap()).decimal(),
            "1234"
        );
    }
    #[test]
    fn mismatches_overflows_and_implicit_rounding_are_rejected() {
        let euro = Currency::eur();
        for s in [
            "1",
            "1.2",
            "1.234",
            "1e2",
            "NaN",
            "+1.00",
            " 1.00",
            "92233720368547758.08",
        ] {
            assert!(Money::parse(s, euro.clone()).is_err(), "{s}");
        }
        assert!(
            Money::new(i64::MAX, euro.clone())
                .checked_add(&Money::new(1, euro.clone()))
                .is_err()
        );
        assert!(
            Money::new(1, euro.clone())
                .checked_add(&Money::new(1, Currency::new("USD", 2).unwrap()))
                .is_err()
        );
        assert!(
            Money::new(1, euro)
                .checked_add(&Money::new(1, Currency::new("EUR", 3).unwrap()))
                .is_err()
        );
    }
}

#[cfg(test)]
mod decimal_rounding {
    use super::*;
    #[test]
    fn ties_signs_scales_and_overflow_are_exact() {
        for (text, scale, expected) in [
            ("1.005", 2, 101),
            ("-1.005", 2, -101),
            ("1.004999999999", 2, 100),
            ("0.005", 2, 1),
            ("1.5", 0, 2),
            ("1.2345", 3, 1235),
            ("92233720368547758.07", 2, i64::MAX),
            ("-92233720368547758.08", 2, i64::MIN),
        ] {
            assert_eq!(
                Money::rounded(text, Currency::new("EUR", scale).unwrap())
                    .unwrap()
                    .minor(),
                expected
            );
        }
        for text in [
            "NaN",
            "Infinity",
            "1e10",
            "1.2.3",
            "92233720368547758.08",
            "-92233720368547758.09",
        ] {
            assert!(Money::rounded(text, Currency::eur()).is_err());
        }
    }
}

#[cfg(test)]
mod minor_units {
    use super::{Minor, unsigned_sum};

    #[test]
    fn line_totals_sums_and_scales_are_exact_or_refused() {
        let unit = Minor::new(-1999, 2);
        assert_eq!(unit.times(3).unwrap(), Minor::new(-5997, 2));
        assert_eq!(unit.times(0).unwrap(), Minor::zero(2));
        assert!(Minor::new(i64::MAX, 2).times(2).is_err());
        assert_eq!(unit.at_scale(3).unwrap(), Minor::new(-19990, 3));
        assert!(unit.at_scale(1).is_err());
        assert!(unit.checked_add(Minor::new(1, 3)).is_err());
        assert_eq!(unsigned_sum([1, 2, 3]).unwrap(), 6);
        assert!(unsigned_sum([u64::MAX, 1]).is_err());
        assert!(Minor::new(-1, 2).unsigned().is_err());
        assert_eq!(serde_json::to_string(&Minor::new(6000, 2)).unwrap(), "60.0");
        assert_eq!(
            serde_json::to_string(&Minor::new(7123, 3)).unwrap(),
            "7.123"
        );
    }
}
