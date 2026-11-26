//! Bind integer provider receipt amounts and status to the formally checked exact-match predicate.
use crate::domain::amounts::parse_amount;
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::Result;
use veyndo_core::document::{lenient, view};
use veyndo_pricing::verified_kernel;

/// The JSON fields `receipt_matches` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReceiptMatchesInput {
    #[serde(deserialize_with = "lenient")]
    value: Option<String>,
    #[serde(rename = "currency_code")]
    currency_code: Value,
}

pub fn receipt_matches(
    expected: i64,
    amount: &Value,
    currency: &str,
    confirmed: bool,
) -> Result<bool> {
    let input: ReceiptMatchesInput = view(amount);
    let received = parse_amount(input.value.as_deref().unwrap_or(""), currency)?;
    Ok(u64::try_from(expected)
        .ok()
        .zip(u64::try_from(received).ok())
        .is_some_and(|(expected, received)| {
            verified_kernel::receipt_admissible(
                expected,
                received,
                input.currency_code == currency,
                confirmed,
            )
        }))
}

#[cfg(test)]
mod tests {
    use super::receipt_matches;
    use serde_json::json;

    #[test]
    fn a_receipt_matches_only_the_exact_confirmed_amount_in_its_currency() {
        let amount = json!({"value":"12.50","currency_code":"EUR"});
        assert_eq!(receipt_matches(1250, &amount, "EUR", true).ok(), Some(true));
        assert_eq!(
            receipt_matches(1250, &amount, "EUR", false).ok(),
            Some(false)
        );
        assert_eq!(
            receipt_matches(1249, &amount, "EUR", true).ok(),
            Some(false)
        );
        assert_eq!(
            receipt_matches(-1250, &amount, "EUR", true).ok(),
            Some(false)
        );
        let foreign = json!({"value":"12.50","currency_code":"USD"});
        assert_eq!(
            receipt_matches(1250, &foreign, "EUR", true).ok(),
            Some(false)
        );
    }

    #[test]
    fn an_unreadable_amount_is_an_error_not_a_mismatch() {
        for amount in [
            json!({}),
            json!({"value":"12.505","currency_code":"EUR"}),
            json!({"value":"-1.00","currency_code":"EUR"}),
        ] {
            assert!(
                receipt_matches(1250, &amount, "EUR", true).is_err(),
                "{amount}"
            );
        }
    }
}
