//! The PostgreSQL `numeric` codec of exact product prices and rates: reads and binds a `numeric` column as the pricing [`Decimal`], without a floating-point intermediary.
//!
//! The binary `numeric` format is `ndigits`, `weight`, `sign`, `dscale` (four big-endian 16-bit
//! words) followed by `ndigits` base-10000 digits; the value is the sum of `digit_i *
//! 10000^(weight - i)`. NaN and infinities have no decimal and are refused.
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef};
use sqlx::{Decode, Encode, Postgres, Type};
use veyndo_pricing::decimal::Decimal;

/// An exact `numeric` column value; checked queries name it with `AS "column!: Exact"` and bind it with `value as Exact`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exact(pub Decimal);

const POSITIVE: u16 = 0x0000;
const NEGATIVE: u16 = 0x4000;
const BASE: i128 = 10_000;
/// The largest decimal scale [`Decimal`] keeps.
const MAX_SCALE: u16 = 38;

impl Type<Postgres> for Exact {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("numeric")
    }
}

fn word(bytes: &[u8], index: usize) -> Result<u16, BoxDynError> {
    let pair = bytes
        .get(index * 2..index * 2 + 2)
        .ok_or("Truncated numeric value")?;
    Ok(u16::from_be_bytes([pair[0], pair[1]]))
}

/// The decimal of a binary `numeric` value.
pub fn from_binary(bytes: &[u8]) -> Result<Decimal, BoxDynError> {
    let digits = usize::from(word(bytes, 0)?);
    let weight = i32::from(word(bytes, 1)? as i16);
    let sign = word(bytes, 2)?;
    let scale = word(bytes, 3)?;
    if sign != POSITIVE && sign != NEGATIVE {
        return Err("A numeric NaN or infinity is not a decimal amount".into());
    }
    if scale > MAX_SCALE {
        return Err("Numeric scale beyond the exact decimal range".into());
    }
    let mut units: i128 = 0;
    for i in 0..digits {
        let digit = i128::from(word(bytes, 4 + i)?);
        // This digit's power of ten at the display scale: 4 per base-10000 position.
        let exponent = 4 * (weight - i as i32) + i32::from(scale);
        let term = if exponent >= 0 {
            10_i128
                .checked_pow(exponent as u32)
                .and_then(|p| digit.checked_mul(p))
                .ok_or("Numeric value beyond the exact decimal range")?
        } else {
            // Digits past the display scale are zeros of the last base-10000 group.
            let divisor = 10_i128
                .checked_pow(exponent.unsigned_abs())
                .ok_or("Numeric value beyond the exact decimal range")?;
            if digit % divisor != 0 {
                return Err("Numeric digits beyond its display scale".into());
            }
            digit / divisor
        };
        units = units
            .checked_add(term)
            .ok_or("Numeric value beyond the exact decimal range")?;
    }
    if sign == NEGATIVE {
        units = -units;
    }
    Ok(Decimal::new(units, u32::from(scale))?.normalized())
}

/// The binary `numeric` value of a decimal, at the decimal's own scale.
pub fn to_binary(value: Decimal) -> Result<Vec<u8>, BoxDynError> {
    let scale = u16::try_from(value.scale()).map_err(|_| "Decimal scale too large")?;
    // Align the units to whole base-10000 groups after the decimal point.
    let pad = (4 - value.scale() % 4) % 4;
    let mut magnitude = value
        .units()
        .unsigned_abs()
        .checked_mul(10_u128.pow(pad))
        .ok_or("Decimal value beyond the numeric range")?;
    let fraction_groups = ((value.scale() + pad) / 4) as i32;
    let mut groups = Vec::new();
    while magnitude > 0 {
        groups.push((magnitude % BASE as u128) as i16);
        magnitude /= BASE as u128;
    }
    // Most significant first; the weight is the position of the first group.
    groups.reverse();
    let weight = groups.len() as i32 - 1 - fraction_groups;
    while groups.last() == Some(&0) {
        groups.pop();
    }
    let mut out = Vec::with_capacity(8 + 2 * groups.len());
    out.extend((groups.len() as u16).to_be_bytes());
    out.extend((if groups.is_empty() { 0 } else { weight as i16 }).to_be_bytes());
    out.extend(
        (if value.units() < 0 {
            NEGATIVE
        } else {
            POSITIVE
        })
        .to_be_bytes(),
    );
    out.extend(scale.to_be_bytes());
    for group in groups {
        out.extend(group.to_be_bytes());
    }
    Ok(out)
}

impl Decode<'_, Postgres> for Exact {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(Exact(match value.format() {
            PgValueFormat::Binary => from_binary(value.as_bytes()?)?,
            PgValueFormat::Text => Decimal::parse(value.as_str()?)?,
        }))
    }
}

impl Encode<'_, Postgres> for Exact {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.extend(to_binary(self.0)?);
        Ok(IsNull::No)
    }
}

/// JSON writes the value as its decimal's number, like the product contract.
impl serde::Serialize for Exact {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl From<Decimal> for Exact {
    fn from(value: Decimal) -> Self {
        Exact(value)
    }
}

impl From<Exact> for Decimal {
    fn from(value: Exact) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str) {
        let value = Decimal::parse(text).unwrap();
        assert_eq!(
            from_binary(&to_binary(value).unwrap()).unwrap(),
            value,
            "{text}"
        );
    }

    #[test]
    fn decimals_round_trip_through_the_binary_format() {
        for text in [
            "0",
            "0.5",
            "1",
            "19.99",
            "12345.67",
            "-12345.67",
            "0.0001",
            "0.00001",
            "1000000",
            "10000",
            "0.1234567890123456",
            "99999999.99999999",
            "-0.3",
            "0.30000000000000004",
        ] {
            round_trip(text);
        }
    }

    #[test]
    fn postgres_encodings_decode_to_their_decimal() {
        // 12345.67: ndigits 3, weight 1, positive, dscale 2, digits 1 2345 6700.
        let bytes = [0, 3, 0, 1, 0, 0, 0, 2, 0, 1, 0x09, 0x29, 0x1a, 0x2c];
        assert_eq!(
            from_binary(&bytes).unwrap(),
            Decimal::parse("12345.67").unwrap()
        );
        // 179.00 keeps its display scale in PostgreSQL; the decimal is 179.
        let bytes = [0, 1, 0, 0, 0, 0, 0, 2, 0, 179];
        assert_eq!(from_binary(&bytes).unwrap(), Decimal::integer(179));
        // NaN is refused.
        assert!(from_binary(&[0, 0, 0, 0, 0xc0, 0, 0, 0]).is_err());
    }
}
