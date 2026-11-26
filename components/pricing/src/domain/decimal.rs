//! Exact decimals and rationals for prices, rates and percentages; the one conversion between JSON numbers and exact values.
//!
//! JSON carries amounts, rates and percentages as binary64 numbers. The one boundary
//! conversion is [`Decimal::from_wire`]: a number reads as the shortest decimal that names
//! it (the digits the JSON text carried), and [`Decimal::to_wire`] writes a decimal as the
//! nearest number. Every calculation runs on [`Decimal`], [`Ratio`] and
//! [`crate::money::Minor`] with checked integer arithmetic; no floating-point arithmetic.
use crate::money::Minor;
use crate::verified_kernel;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;

/// Why an exact amount could not be formed or calculated.
pub type MoneyError = &'static str;

const OVERFLOW: MoneyError = "Amount overflow";

/// An exact decimal number: `units / 10^scale`.
#[derive(Clone, Copy, Debug)]
pub struct Decimal {
    units: i128,
    scale: u32,
}

/// The largest scale a decimal keeps; 10^38 is the largest power of ten in `i128`.
const MAX_SCALE: u32 = 38;

fn power(exponent: u32) -> Result<i128, MoneyError> {
    10_i128.checked_pow(exponent).ok_or(OVERFLOW)
}

impl Decimal {
    pub const ZERO: Self = Self { units: 0, scale: 0 };

    pub fn new(units: i128, scale: u32) -> Result<Self, MoneyError> {
        if scale > MAX_SCALE {
            return Err(OVERFLOW);
        }
        Ok(Self { units, scale })
    }
    pub fn integer(value: i64) -> Self {
        Self {
            units: value.into(),
            scale: 0,
        }
    }
    pub fn units(&self) -> i128 {
        self.units
    }
    pub fn scale(&self) -> u32 {
        self.scale
    }
    pub fn ratio(&self) -> Ratio {
        // 10^scale fits for every admitted scale.
        Ratio::reduced(self.units, 10_i128.pow(self.scale))
    }
    pub fn is_zero(&self) -> bool {
        self.units == 0
    }
    /// Plain decimal text: an optional minus, digits and an optional fraction; no exponent.
    pub fn parse(text: &str) -> Result<Self, MoneyError> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        if whole.is_empty()
            || !whole.bytes().all(|b| b.is_ascii_digit())
            || !fraction.bytes().all(|b| b.is_ascii_digit())
            || (digits.contains('.') && fraction.is_empty())
        {
            return Err("Decimal amount required");
        }
        let fraction = fraction.trim_end_matches('0');
        let scale = u32::try_from(fraction.len()).map_err(|_| OVERFLOW)?;
        let mut units: i128 = 0;
        for digit in whole.bytes().chain(fraction.bytes()) {
            units = units
                .checked_mul(10)
                .and_then(|u| u.checked_add(i128::from(digit - b'0')))
                .ok_or(OVERFLOW)?;
        }
        Self::new(if negative { -units } else { units }, scale)
    }
    /// The decimal digits, without exponent and without trailing fraction zeros.
    pub fn text(&self) -> String {
        let sign = if self.units < 0 { "-" } else { "" };
        let digits = self.units.unsigned_abs().to_string();
        let scale = self.scale as usize;
        let (whole, fraction) = if digits.len() > scale {
            let (w, f) = digits.split_at(digits.len() - scale);
            (w.to_string(), f.to_string())
        } else {
            ("0".to_string(), format!("{digits:0>scale$}"))
        };
        let fraction = fraction.trim_end_matches('0');
        if fraction.is_empty() {
            format!("{sign}{whole}")
        } else {
            format!("{sign}{whole}.{fraction}")
        }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.ratio() == other.ratio()
    }
}
impl Eq for Decimal {}
impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ratio().cmp(&other.ratio())
    }
}

impl Decimal {
    /// The boundary conversion from a JSON number: the shortest decimal that reads back as
    /// the same binary64 number (the digits the JSON text carried). Rust writes a finite
    /// `f64` as exactly these digits, without exponent; NaN, infinities and values outside
    /// the decimal range (more than 38 decimals or digits) are refused.
    pub fn from_wire(value: f64) -> Result<Self, MoneyError> {
        if !value.is_finite() {
            return Err("Invalid amount");
        }
        Self::parse(&value.to_string()).map_err(|_| "Invalid amount")
    }
    /// The boundary conversion to a JSON number: the binary64 number nearest the exact
    /// value (Rust's correctly rounded decimal reading).
    pub fn to_wire(&self) -> Result<f64, MoneyError> {
        self.text()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or(OVERFLOW)
    }
    /// The exact sum at the larger scale; refused on overflow.
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        let scale = self.scale.max(other.scale);
        let at = |d: Self| d.units.checked_mul(power(scale - d.scale)?).ok_or(OVERFLOW);
        Self::new(at(self)?.checked_add(at(other)?).ok_or(OVERFLOW)?, scale)
    }
    /// The decimal without trailing fraction zeros (`1.50` → `1.5`, scale 1).
    pub fn normalized(self) -> Self {
        let (mut units, mut scale) = (self.units, self.scale);
        while scale > 0 && units % 10 == 0 {
            units /= 10;
            scale -= 1;
        }
        Self { units, scale }
    }
}

/// JSON reads and writes a decimal through its binary64 number, losslessly.
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.to_wire().map_err(serde::ser::Error::custom)?)
    }
}
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_wire(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
/// JSON reads a rational input as the decimal its number carried.
impl<'de> Deserialize<'de> for Ratio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Decimal::deserialize(deserializer)?.ratio())
    }
}

/// An exact rational number in lowest terms with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ratio {
    numerator: i128,
    denominator: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl Ratio {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    /// `numerator / denominator` in lowest terms; the denominator must be positive.
    fn reduced(numerator: i128, denominator: i128) -> Self {
        let g = gcd(numerator, denominator);
        Self {
            numerator: numerator / g,
            denominator: denominator / g,
        }
    }
    pub fn new(numerator: i128, denominator: i128) -> Result<Self, MoneyError> {
        match denominator.cmp(&0) {
            Ordering::Equal => Err("Division by zero"),
            Ordering::Greater => Ok(Self::reduced(numerator, denominator)),
            Ordering::Less => Ok(Self::reduced(
                numerator.checked_neg().ok_or(OVERFLOW)?,
                denominator.checked_neg().ok_or(OVERFLOW)?,
            )),
        }
    }
    pub fn integer(value: i128) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }
    pub fn numerator(&self) -> i128 {
        self.numerator
    }
    pub fn denominator(&self) -> i128 {
        self.denominator
    }
    pub fn is_zero(&self) -> bool {
        self.numerator == 0
    }
    pub fn is_negative(&self) -> bool {
        self.numerator < 0
    }
    pub fn negated(self) -> Result<Self, MoneyError> {
        Ok(Self {
            numerator: self.numerator.checked_neg().ok_or(OVERFLOW)?,
            denominator: self.denominator,
        })
    }
    pub fn abs(self) -> Result<Self, MoneyError> {
        if self.is_negative() {
            self.negated()
        } else {
            Ok(self)
        }
    }
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        let g = gcd(self.denominator, other.denominator);
        let left = self
            .numerator
            .checked_mul(other.denominator / g)
            .ok_or(OVERFLOW)?;
        let right = other
            .numerator
            .checked_mul(self.denominator / g)
            .ok_or(OVERFLOW)?;
        let denominator = (self.denominator / g)
            .checked_mul(other.denominator)
            .ok_or(OVERFLOW)?;
        Ok(Self::reduced(
            left.checked_add(right).ok_or(OVERFLOW)?,
            denominator,
        ))
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.checked_add(other.negated()?)
    }
    pub fn checked_mul(self, other: Self) -> Result<Self, MoneyError> {
        // Cross-reduce first so exact products of reduced factors stay small.
        let a = gcd(self.numerator, other.denominator);
        let b = gcd(other.numerator, self.denominator);
        let numerator = (self.numerator / a)
            .checked_mul(other.numerator / b)
            .ok_or(OVERFLOW)?;
        let denominator = (self.denominator / b)
            .checked_mul(other.denominator / a)
            .ok_or(OVERFLOW)?;
        Ok(Self::reduced(numerator, denominator))
    }
    pub fn checked_div(self, other: Self) -> Result<Self, MoneyError> {
        if other.is_zero() {
            return Err("Division by zero");
        }
        self.checked_mul(Self::new(other.denominator, other.numerator)?)
    }
    /// The value rounded to `digits` significant decimal digits, ties to even (how a
    /// converted snapshot price is kept so its binary64 number reads back as that decimal).
    pub fn significant(self, digits: u32) -> Result<Decimal, MoneyError> {
        self.significant_with(digits, false)
    }
    /// The value rounded to `digits` significant decimal digits, ties away from zero (how
    /// the shortest digits of a number are chosen when two are equally close).
    pub fn significant_away(self, digits: u32) -> Result<Decimal, MoneyError> {
        self.significant_with(digits, true)
    }
    fn significant_with(self, digits: u32, away: bool) -> Result<Decimal, MoneyError> {
        if self.is_zero() {
            return Ok(Decimal::ZERO);
        }
        let negative = self.is_negative();
        let numerator = self.numerator.unsigned_abs();
        let denominator = self.denominator.unsigned_abs();
        // Long division: the integer part, then fraction digits until `digits`
        // significant ones and one more are known; the remainder is the sticky part.
        let whole = numerator / denominator;
        let mut remainder = numerator % denominator;
        let mut mantissa: u128 = whole;
        let mut kept = if whole == 0 { 0 } else { whole.ilog10() + 1 };
        let mut scale: u32 = 0;
        while kept < digits + 1 {
            remainder = remainder.checked_mul(10).ok_or(OVERFLOW)?;
            let digit = remainder / denominator;
            remainder %= denominator;
            mantissa = mantissa
                .checked_mul(10)
                .and_then(|m| m.checked_add(digit))
                .ok_or(OVERFLOW)?;
            scale += 1;
            if mantissa != 0 {
                kept += 1;
            }
        }
        // `mantissa` holds `kept` significant digits at `scale`; drop the extra ones.
        let drop = kept - digits;
        let divisor = 10_u128.checked_pow(drop).ok_or(OVERFLOW)?;
        let (mut rounded, tail) = (mantissa / divisor, mantissa % divisor);
        let half = divisor / 2;
        if tail > half || (tail == half && (remainder != 0 || away || rounded % 2 == 1)) {
            rounded += 1;
        }
        let units = i128::try_from(rounded).map_err(|_| OVERFLOW)?;
        let units = if negative { -units } else { units };
        if scale >= drop {
            Decimal::new(units, scale - drop)
        } else {
            let factor = power(drop - scale)?;
            Decimal::new(units.checked_mul(factor).ok_or(OVERFLOW)?, 0)
        }
    }
    /// The value in whole minor units of `scale`, rounded half away from zero.
    pub fn rounded(self, scale: u8) -> Result<Minor, MoneyError> {
        let factor = power(scale.into())?;
        // Only the factor's part the denominator does not cancel multiplies the magnitude.
        let g = gcd(factor, self.denominator);
        let numerator = self
            .numerator
            .unsigned_abs()
            .checked_mul((factor / g).unsigned_abs())
            .ok_or(OVERFLOW)?;
        let denominator = (self.denominator / g).unsigned_abs();
        let magnitude = quotient(numerator, denominator)?;
        let minor = i64::try_from(magnitude).map_err(|_| OVERFLOW)?;
        Ok(Minor::new(
            if self.is_negative() { -minor } else { minor },
            scale,
        ))
    }
}

/// `numerator / denominator` rounded half up through the proved quotient rule; refused
/// when the operands exceed the rule's range instead of saturating.
///
/// A numerator beyond the rule's range is split into whole units and a remainder below the
/// denominator: `(n + d/2) / d = n / d + (n mod d + d/2) / d`, so the proved rule still
/// decides the rounding of the remainder.
fn quotient(numerator: u128, denominator: u128) -> Result<u64, MoneyError> {
    let divisor = u64::try_from(denominator).map_err(|_| OVERFLOW)?;
    let proved = |n: u64| {
        n.checked_add(divisor / 2)
            .map(|_| verified_kernel::rounded_quotient(n, divisor))
            .ok_or(OVERFLOW)
    };
    match u64::try_from(numerator) {
        Ok(n) if n.checked_add(divisor / 2).is_some() => proved(n),
        _ => {
            let whole = u64::try_from(numerator / denominator).map_err(|_| OVERFLOW)?;
            let rest = u64::try_from(numerator % denominator).map_err(|_| OVERFLOW)?;
            whole.checked_add(proved(rest)?).ok_or(OVERFLOW)
        }
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        // Signs first, then the magnitudes' cross products in 256 bits: exact for every value.
        let sign = |r: &Self| r.numerator.cmp(&0);
        match sign(self).cmp(&sign(other)) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        let left = wide(
            self.numerator.unsigned_abs(),
            other.denominator.unsigned_abs(),
        );
        let right = wide(
            other.numerator.unsigned_abs(),
            self.denominator.unsigned_abs(),
        );
        if self.is_negative() {
            right.cmp(&left)
        } else {
            left.cmp(&right)
        }
    }
}

/// The full 256-bit product of two magnitudes as (high, low) halves.
fn wide(a: u128, b: u128) -> (u128, u128) {
    let mask = u128::from(u64::MAX);
    let (a1, a0) = (a >> 64, a & mask);
    let (b1, b0) = (b >> 64, b & mask);
    let low = a0 * b0;
    let middle_a = a1 * b0;
    let middle_b = a0 * b1;
    let high = a1 * b1;
    let (middle, carry) = middle_a.overflowing_add(middle_b);
    let (low, carry_low) = low.overflowing_add(middle << 64);
    let high = high + (middle >> 64) + (u128::from(carry) << 64) + u128::from(carry_low);
    (high, low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        Decimal::parse(text).unwrap()
    }

    #[test]
    fn fourteen_significant_digits_round_ties_to_even() {
        let r = |n: i128, m: i128| Ratio::new(n, m).unwrap();
        assert_eq!(
            r(1999, 100)
                .checked_mul(r(19, 119))
                .unwrap()
                .significant(14)
                .unwrap()
                .text(),
            "3.1916806722689"
        );
        assert_eq!(r(2, 3).significant(14).unwrap().text(), "0.66666666666667");
        assert_eq!(
            d("123456789012345").ratio().significant(14).unwrap().text(),
            "123456789012340"
        );
        assert_eq!(
            d("123456789012355").ratio().significant(14).unwrap().text(),
            "123456789012360"
        );
        assert_eq!(
            d("-0.095").ratio().significant(14).unwrap().text(),
            "-0.095"
        );
        assert_eq!(Ratio::ZERO.significant(14).unwrap(), Decimal::ZERO);
    }

    #[test]
    fn rounding_is_half_away_from_zero() {
        for (text, scale, minor) in [
            ("1.005", 2, 101),
            ("-1.005", 2, -101),
            ("1.004999", 2, 100),
            ("0.095", 2, 10),
            ("7.4", 0, 7),
            ("-7.5", 0, -8),
        ] {
            assert_eq!(d(text).ratio().rounded(scale).unwrap().minor(), minor);
        }
    }

    #[test]
    fn arithmetic_is_exact_or_refused() {
        let third = Ratio::new(1, 3).unwrap();
        assert_eq!(
            third
                .checked_add(third)
                .unwrap()
                .checked_add(third)
                .unwrap(),
            Ratio::integer(1)
        );
        assert!(
            Ratio::integer(i128::MAX)
                .checked_add(Ratio::integer(1))
                .is_err()
        );
        assert!(third.checked_div(Ratio::ZERO).is_err());
        assert!(Ratio::new(1, 0).is_err());
        assert!(d("2.5") > d("2.49"));
        assert_eq!(d("2.50"), d("2.5"));
    }

    #[test]
    fn wire_numbers_read_as_their_shortest_decimal_and_back() {
        for (value, text) in [
            (19.99, "19.99"),
            (0.1 + 0.2, "0.30000000000000004"),
            (-1.005, "-1.005"),
            (1584.0, "1584"),
            (0.0, "0"),
            (-0.0, "0"),
            (1e-7, "0.0000001"),
            (123456789012.345, "123456789012.345"),
            (130.4835, "130.4835"),
        ] {
            let exact = Decimal::from_wire(value).unwrap();
            assert_eq!(exact.text(), text);
            assert_eq!(exact.to_wire().unwrap().to_bits(), (value + 0.).to_bits());
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, 1e-40] {
            assert!(Decimal::from_wire(value).is_err());
        }
        assert_eq!(Decimal::new(150, 2).unwrap().normalized().scale(), 1);
        assert_eq!(d("0.1").checked_add(d("0.2")).unwrap(), d("0.3"));
        assert_eq!(d("-1.005").checked_add(d("2")).unwrap().text(), "0.995");
        assert!(
            Decimal::new(i128::MAX, 0)
                .unwrap()
                .checked_add(d("1"))
                .is_err()
        );
        assert_eq!(Decimal::new(0, 3).unwrap().normalized().scale(), 0);
    }

    #[test]
    fn large_numerators_still_round_through_the_proved_rule() {
        // Numerators beyond the proved rule's 64-bit range: whole units plus a rounded remainder.
        let big = Ratio::new((1_i128 << 71) + 1, 2).unwrap();
        assert!(big.rounded(0).is_err(), "beyond i64 minor units");
        let value = Ratio::new(20_000_000_000_000_000_001, 2_000).unwrap();
        assert_eq!(value.rounded(0).unwrap().minor(), 10_000_000_000_000_000);
        assert_eq!(
            value.negated().unwrap().rounded(0).unwrap().minor(),
            -10_000_000_000_000_000
        );
        let up = Ratio::new(20_000_000_000_000_001_999, 2_000).unwrap();
        assert_eq!(up.rounded(0).unwrap().minor(), 10_000_000_000_000_001);
    }
}
