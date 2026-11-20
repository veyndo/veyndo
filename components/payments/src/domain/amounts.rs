//! Provider wire amounts: decimal strings at the ISO precision of their currency, exact in minor units.
use veyndo_core::{Result, bad};
use veyndo_pricing::money::{Currency, Money};
use veyndo_pricing::types::currency;

pub fn wire_currency(code: &str) -> Result<Currency> {
    Currency::new(
        code,
        currency::scale(code).ok_or(bad("Unsupported provider currency"))?,
    )
    .map_err(bad)
}
pub fn format_amount(minor: i64, code: &str) -> Result<String> {
    Ok(Money::new(minor, wire_currency(code)?).decimal())
}
pub fn parse_amount(value: &str, code: &str) -> Result<i64> {
    let money = Money::parse(value, wire_currency(code)?).map_err(bad)?;
    if money.minor() < 0 || value.starts_with('-') {
        return Err(bad("Invalid provider money amount"));
    }
    Ok(money.minor())
}
#[cfg(test)]
fn amount_string(minor: i64) -> String {
    Money::new(minor, Currency::eur()).decimal()
}
#[cfg(test)]
fn parse_minor(value: &str) -> Result<i64> {
    let amount = Money::parse(value, Currency::eur()).map_err(bad)?;
    if amount.minor() < 0 || value.starts_with('-') {
        return Err(bad("Invalid provider money amount"));
    }
    Ok(amount.minor())
}

#[cfg(test)]
mod tests {
    use super::{amount_string, format_amount, parse_amount, parse_minor, wire_currency};

    #[test]
    fn currency_wire_amounts_use_iso_precision() {
        assert_eq!(format_amount(123, "JPY").unwrap(), "123");
        assert_eq!(format_amount(7123, "KWD").unwrap(), "7.123");
        assert_eq!(parse_amount("7.123", "KWD").unwrap(), 7123);
        assert!(parse_amount("1.001", "USD").is_err());
        assert!(parse_amount("1.00", "XXX").is_err());
    }
    #[test]
    fn integer_money_roundtrips() {
        for n in [1, 99, 100, 7490, 1000000] {
            assert_eq!(parse_minor(&amount_string(n)).unwrap(), n);
        }
        for s in ["1.2", "-1.00", "NaN", "1e2.00", "999999999999999999999.00"] {
            assert!(parse_minor(s).is_err());
        }
    }
    #[test]
    fn wire_currencies_are_the_supported_iso_codes() {
        assert!(wire_currency("EUR").is_ok());
        assert!(wire_currency("JPY").is_ok());
        for code in ["XXX", "eur", ""] {
            assert!(wire_currency(code).is_err(), "{code}");
        }
        assert!(parse_amount("-0.00", "EUR").is_err());
    }
}
