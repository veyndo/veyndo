//! The original calculators' binary arithmetic, reproduced exactly with integers.
//!
//! Shopware computes prices in IEEE-754 binary64 and rounds the binary results. At a
//! decimal tie the binary error decides the direction: 130.4835 × 670 is 87423.945 in
//! decimals, but 87423.94499999999 in binary64, so the original rounds it to 87423.94.
//! To return the original's amounts for every input, the port performs the same binary
//! steps. [`Binary`] is a binary64 value as an integer significand and a power-of-two
//! exponent; addition, subtraction, multiplication and division are the exact result
//! rounded to the nearest representable value, ties to even, as IEEE-754 prescribes.
//! No floating-point type is involved. Values outside the normal range (subnormals,
//! infinities, NaN) are refused instead of emulated: no price reaches them.
use crate::decimal::{Decimal, MoneyError, Ratio};
use crate::money::Minor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;

const OVERFLOW: MoneyError = "Amount overflow";
/// Significand bits of binary64, including the implicit leading one.
const BITS: u32 = 53;
/// The smallest and largest exponent of a normal binary64 value's least significant bit.
const MIN_EXPONENT: i32 = -1074;
const MAX_EXPONENT: i32 = 971;

/// A binary64 value: `significand * 2^exponent`, the significand zero or in `[2^52, 2^53)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binary {
    negative: bool,
    significand: u64,
    exponent: i32,
}

fn bit_length(value: u128) -> u32 {
    128 - value.leading_zeros()
}

impl Binary {
    pub const ZERO: Self = Self {
        negative: false,
        significand: 0,
        exponent: 0,
    };

    /// `±magnitude * 2^exponent` rounded to 53 bits, ties to even; `sticky` marks a nonzero
    /// remainder below `magnitude`'s last bit.
    fn rounded(
        negative: bool,
        magnitude: u128,
        exponent: i32,
        sticky: bool,
    ) -> Result<Self, MoneyError> {
        if magnitude == 0 {
            return Ok(Self::ZERO);
        }
        let length = bit_length(magnitude);
        let (mut significand, mut exponent) = if length > BITS {
            let shift = length - BITS;
            let kept = magnitude >> shift;
            let rest = magnitude & ((1_u128 << shift) - 1);
            let half = 1_u128 << (shift - 1);
            let up = rest > half || (rest == half && (sticky || kept & 1 == 1));
            (kept + u128::from(up), exponent + shift as i32)
        } else {
            if sticky {
                // Callers always supply at least two bits beyond the significand.
                return Err(OVERFLOW);
            }
            (
                magnitude << (BITS - length),
                exponent - (BITS - length) as i32,
            )
        };
        if significand == 1 << BITS {
            significand >>= 1;
            exponent += 1;
        }
        if !(MIN_EXPONENT..=MAX_EXPONENT).contains(&exponent) {
            return Err(OVERFLOW);
        }
        Ok(Self {
            negative,
            significand: significand as u64,
            exponent,
        })
    }
    pub fn integer(value: i64) -> Self {
        // Integers below 2^53 are exact; larger ones round like the original's conversion.
        Self::rounded(value < 0, value.unsigned_abs().into(), 0, false).unwrap_or(Self::ZERO)
    }
    /// The binary64 value nearest an exact rational, ties to even (a correctly rounded conversion).
    pub fn from_ratio(value: Ratio) -> Result<Self, MoneyError> {
        let negative = value.is_negative();
        let numerator = value.numerator().unsigned_abs();
        let denominator = value.denominator().unsigned_abs();
        let mut quotient = numerator / denominator;
        let mut remainder = numerator % denominator;
        let mut exponent = 0;
        // Binary long division until two bits beyond the significand are known.
        while bit_length(quotient) < BITS + 2 {
            if exponent < MIN_EXPONENT - 64 {
                return Err(OVERFLOW);
            }
            let doubled = remainder << 1;
            let bit = doubled >= denominator;
            remainder = if bit { doubled - denominator } else { doubled };
            quotient = (quotient << 1) | u128::from(bit);
            exponent -= 1;
            if quotient == 0 && remainder == 0 {
                return Ok(Self::ZERO);
            }
        }
        Self::rounded(negative, quotient, exponent, remainder != 0)
    }
    /// The binary64 value nearest an exact decimal (how a decimal literal or JSON number reads).
    pub fn from_decimal(value: Decimal) -> Result<Self, MoneyError> {
        Self::from_ratio(value.ratio())
    }
    /// The exact value; refused when it needs more than 126 binary places or 127 bits.
    pub fn ratio(&self) -> Result<Ratio, MoneyError> {
        let magnitude = i128::from(self.significand);
        let value = if self.exponent >= 0 {
            Ratio::integer(
                magnitude
                    .checked_mul(1_i128.checked_shl(self.exponent as u32).ok_or(OVERFLOW)?)
                    .filter(|_| self.exponent < 127 - BITS as i32)
                    .ok_or(OVERFLOW)?,
            )
        } else {
            let shift = u32::try_from(-self.exponent).map_err(|_| OVERFLOW)?;
            if shift > 126 {
                return Err(OVERFLOW);
            }
            Ratio::new(magnitude, 1_i128 << shift)?
        };
        if self.negative {
            value.negated()
        } else {
            Ok(value)
        }
    }
    pub fn is_zero(&self) -> bool {
        self.significand == 0
    }
    pub fn is_negative(&self) -> bool {
        self.negative && !self.is_zero()
    }
    pub fn negated(self) -> Self {
        if self.is_zero() {
            self
        } else {
            Self {
                negative: !self.negative,
                ..self
            }
        }
    }
    pub fn abs(self) -> Self {
        Self {
            negative: false,
            ..self
        }
    }
    pub fn checked_mul(self, other: Self) -> Result<Self, MoneyError> {
        if self.is_zero() || other.is_zero() {
            return Ok(Self::ZERO);
        }
        let product = u128::from(self.significand) * u128::from(other.significand);
        Self::rounded(
            self.negative != other.negative,
            product,
            self.exponent + other.exponent,
            false,
        )
    }
    pub fn checked_div(self, other: Self) -> Result<Self, MoneyError> {
        if other.is_zero() {
            return Err("Division by zero");
        }
        if self.is_zero() {
            return Ok(Self::ZERO);
        }
        // 64 extra bits leave at least 64 + 52 - 53 + 1 = 64 bits of quotient beyond the significand.
        let dividend = u128::from(self.significand) << 64;
        let divisor = u128::from(other.significand);
        Self::rounded(
            self.negative != other.negative,
            dividend / divisor,
            self.exponent - other.exponent - 64,
            dividend % divisor != 0,
        )
    }
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.is_zero() {
            return Ok(other);
        }
        if other.is_zero() {
            return Ok(self);
        }
        let (large, small) = if self.abs() >= other.abs() {
            (self, other)
        } else {
            (other, self)
        };
        let gap = (large.exponent - small.exponent) as u32;
        if gap > 70 {
            // The smaller value is below a quarter of the larger one's last bit: the larger
            // value is the nearest result (also below a power of two, where the grid halves).
            return Ok(large);
        }
        let left = u128::from(large.significand) << gap;
        let right = u128::from(small.significand);
        let magnitude = if large.negative == small.negative {
            left + right
        } else {
            left - right
        };
        Self::rounded(large.negative, magnitude, small.exponent, false)
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.checked_add(other.negated())
    }
    /// The largest integer not above the value (`f64::floor`).
    pub fn floor(self) -> Result<Self, MoneyError> {
        if self.is_zero() || self.exponent >= 0 {
            return Ok(self);
        }
        let shift = (-self.exponent) as u32;
        let (whole, exact) = if shift >= 64 {
            (0, false)
        } else {
            (
                self.significand >> shift,
                self.significand & ((1_u64 << shift) - 1) == 0,
            )
        };
        let whole = if self.negative && !exact {
            whole + 1
        } else {
            whole
        };
        Self::rounded(self.negative, whole.into(), 0, false)
    }
    /// The original's fourteen-digit precision step (`format!("{x:.13e}")` read back).
    pub fn cast(self) -> Result<Self, MoneyError> {
        Self::from_decimal(self.decimal()?)
    }
    /// The value at fourteen significant digits, ties to even: the decimal the original writes.
    pub fn decimal(&self) -> Result<Decimal, MoneyError> {
        self.ratio()?.significant(14)
    }
    /// The amount the original writes for this value, in minor units of `scale`.
    pub fn minor(&self, scale: u8) -> Result<Minor, MoneyError> {
        let decimal = self.decimal()?;
        let minor = Minor::from_decimal(decimal, scale)?;
        if minor.ratio() != decimal.ratio() {
            return Err("Amount precision exceeds the currency scale");
        }
        Ok(minor)
    }
    /// The shortest decimal that reads back as this value (how JSON writes a number).
    pub fn shortest(&self) -> Result<Decimal, MoneyError> {
        let exact = self.ratio()?;
        for digits in 1..=17 {
            let candidate = exact.significant_away(digits)?;
            if Self::from_decimal(candidate)? == *self {
                return Ok(candidate);
            }
        }
        Err(OVERFLOW)
    }
    /// The boundary conversion from a JSON number: its exact bits. NaN, infinities and
    /// subnormal values are refused.
    pub fn from_wire(value: f64) -> Result<Self, MoneyError> {
        let bits = value.to_bits();
        let negative = bits >> 63 == 1;
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1 << 52) - 1);
        match biased {
            0 if fraction == 0 => Ok(Self::ZERO),
            0 | 0x7ff => Err("Invalid amount"),
            _ => Ok(Self {
                negative,
                significand: fraction | (1 << 52),
                exponent: biased - 1075,
            }),
        }
    }
    /// The boundary conversion to a JSON number: the same bits.
    pub fn to_wire(&self) -> f64 {
        if self.is_zero() {
            return 0.;
        }
        let sign = u64::from(self.negative) << 63;
        let biased = ((self.exponent + 1075) as u64) << 52;
        f64::from_bits(sign | biased | (self.significand & ((1 << 52) - 1)))
    }
    /// 10^decimals, exact for every supported precision.
    fn power(decimals: u8) -> Result<Self, MoneyError> {
        Ok(Self::integer(
            10_i64.checked_pow(decimals.into()).ok_or(OVERFLOW)?,
        ))
    }
    /// The original `round(value, decimals)`: the floor of the scaled value, corrected when
    /// the next whole unit reads back as the value, then up at or above the binary midpoint.
    pub fn round(self, decimals: u8) -> Result<Self, MoneyError> {
        let scale = Self::power(decimals)?;
        let value = self.abs();
        let one = Self::integer(1);
        let mut whole = value.checked_mul(scale)?.floor()?;
        if whole.checked_add(one)?.checked_div(scale)? == value {
            whole = whole.checked_add(one)?;
        }
        let half = Self::from_ratio(Ratio::new(1, 2)?)?;
        if value >= whole.checked_add(half)?.checked_div(scale)? {
            whole = whole.checked_add(one)?;
        }
        let rounded = whole.checked_div(scale)?;
        Ok(if self.is_negative() {
            rounded.negated()
        } else {
            rounded
        })
    }
    /// The original cash rounding: `round` to the precision, then, up to two decimals, to the
    /// interval through `100 / (interval * 100)`.
    pub fn cash_round(self, decimals: u8, interval: Self) -> Result<Self, MoneyError> {
        let rounded = self.round(decimals)?;
        if decimals > 2 {
            return Ok(rounded);
        }
        let hundred = Self::integer(100);
        let multiplier = hundred.checked_div(interval.checked_mul(hundred)?)?;
        rounded
            .checked_mul(multiplier)?
            .round(0)?
            .checked_div(multiplier)
    }
}

impl Decimal {
    /// The boundary conversion from a JSON number: the shortest decimal that reads back as
    /// the same binary64 number (the digits the JSON text carried). NaN, infinities and
    /// values too small for the amount range are refused.
    pub fn from_wire(value: f64) -> Result<Self, MoneyError> {
        Binary::from_wire(value)?.shortest()
    }
    /// The boundary conversion to a JSON number: the binary64 number nearest the exact value.
    pub fn to_wire(&self) -> Result<f64, MoneyError> {
        Ok(Binary::from_decimal(*self)?.to_wire())
    }
}

impl Serialize for Binary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.to_wire())
    }
}
impl<'de> Deserialize<'de> for Binary {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_wire(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl PartialOrd for Binary {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Binary {
    fn cmp(&self, other: &Self) -> Ordering {
        let magnitude = |b: &Self| (b.significand != 0, b.exponent, b.significand);
        match (self.is_negative(), other.is_negative()) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => magnitude(self).cmp(&magnitude(other)),
            (true, true) => magnitude(other).cmp(&magnitude(self)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(text: &str) -> Binary {
        Binary::from_decimal(Decimal::parse(text).unwrap()).unwrap()
    }
    fn text(value: Binary) -> String {
        value.decimal().unwrap().text()
    }

    #[test]
    fn conversions_are_correctly_rounded() {
        // 0.1 is 0x3FB999999999999A: significand 7205759403792794 at 2^-56.
        assert_eq!(
            b("0.1"),
            Binary {
                negative: false,
                significand: 7_205_759_403_792_794,
                exponent: -56
            }
        );
        assert_eq!(b("1").ratio().unwrap(), Ratio::integer(1));
        assert_eq!(text(b("19.99")), "19.99");
        assert_eq!(Binary::integer(-7).ratio().unwrap(), Ratio::integer(-7));
    }

    #[test]
    fn arithmetic_rounds_like_binary64() {
        // 0.1 + 0.2 = 0.30000000000000004 and 130.4835 * 670 = 87423.94499999999.
        let sum = b("0.1").checked_add(b("0.2")).unwrap();
        assert_eq!(
            sum.ratio().unwrap().significant(17).unwrap().text(),
            "0.30000000000000004"
        );
        let product = b("130.4835").checked_mul(Binary::integer(670)).unwrap();
        assert_eq!(
            product.ratio().unwrap().significant(16).unwrap().text(),
            "87423.94499999999"
        );
        assert_eq!(text(product.round(2).unwrap()), "87423.94");
        assert_eq!(
            text(b("1").checked_div(b("3")).unwrap()),
            "0.33333333333333"
        );
        assert_eq!(
            b("0.3")
                .checked_sub(b("0.1"))
                .unwrap()
                .ratio()
                .unwrap()
                .significant(17)
                .unwrap()
                .text(),
            "0.19999999999999998"
        );
        assert!(b("1").checked_div(Binary::ZERO).is_err());
    }

    #[test]
    fn wire_bits_and_shortest_decimals_round_trip() {
        for value in [
            0.1,
            19.99,
            -1.005,
            1e-7,
            123456789012.345,
            0.1 + 0.2,
            1584.0,
            -0.0,
            2.0_f64.powi(-60),
        ] {
            let binary = Binary::from_wire(value).unwrap();
            assert_eq!(binary.to_wire().to_bits(), (value + 0.).to_bits());
            assert_eq!(binary.shortest().unwrap().text(), format!("{}", value + 0.));
        }
        for value in [f64::NAN, f64::INFINITY, f64::MIN_POSITIVE / 4.] {
            assert!(Binary::from_wire(value).is_err());
        }
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
        ] {
            let exact = Decimal::from_wire(value).unwrap();
            assert_eq!(exact.text(), text);
            assert_eq!(exact.to_wire().unwrap().to_bits(), (value + 0.).to_bits());
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, 1e-40] {
            assert!(Decimal::from_wire(value).is_err());
        }
    }

    #[test]
    fn rounding_follows_the_original() {
        for (value, decimals, rounded) in [
            ("1.005", 2, "1.01"),
            ("-1.005", 2, "-1.01"),
            ("1.004999", 2, "1"),
            ("0.285", 2, "0.29"),
            ("2.5", 0, "3"),
            ("-0.004", 2, "0"),
        ] {
            assert_eq!(text(b(value).round(decimals).unwrap()), rounded, "{value}");
        }
        assert_eq!(text(b("1.03").cash_round(2, b("0.05")).unwrap()), "1.05");
        assert_eq!(text(b("1.02").cash_round(2, b("0.05")).unwrap()), "1");
        assert_eq!(text(b("-2.5").floor().unwrap()), "-3");
        assert_eq!(text(b("2.5").floor().unwrap()), "2");
    }
}
