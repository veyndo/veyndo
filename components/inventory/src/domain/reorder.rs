//! Whether a supplier's signed offer is the reorder the merchant saved, and within the ceiling.
use crate::domain::supply::Link;
use serde_json::Value;
use veyndo_core::{Result, bad, conflict};

/// A proposal line may be approved only in the pricing currency and at or below the ceiling.
pub fn within_ceiling(same_currency: bool, total: Option<u64>, ceiling: Option<u64>) -> bool {
    same_currency
        && total
            .zip(ceiling)
            .is_some_and(|(total, limit)| total <= limit)
}

/// The exact total of an offer that matches the saved link: one line, that product, that quantity, the pricing currency.
pub fn admit(offer: &Value, link: &Link, currency: &str, ceiling: u64) -> Result<u64> {
    let line = &offer["lineItems"];
    let total = offer["totalMinor"]
        .as_str()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(bad("Offer has no exact total"))?;
    if line.as_array().map_or(0, Vec::len) != 1
        || line[0]["id"].as_str() != Some(link.supplier_product_id.as_str())
        || line[0]["quantity"].as_i64() != Some(i64::from(link.reorder_quantity))
        || offer["currency"].as_str() != Some(currency)
    {
        return Err(conflict("The offer does not match the saved supplier link"));
    }
    if total > ceiling {
        return Err(conflict("The offer is above the ceiling per reorder"));
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{admit, within_ceiling};
    use crate::domain::supply::Link;
    use serde_json::json;

    fn link() -> Link {
        Link {
            product_id: "desk".into(),
            supplier_shop: "supplier".into(),
            supplier_product_id: "lamp".into(),
            reorder_point: 5,
            reorder_quantity: 10,
        }
    }

    #[test]
    fn only_the_saved_reorder_within_the_ceiling_is_admitted() {
        let offer =
            json!({"lineItems":[{"id":"lamp","quantity":10}],"totalMinor":"5000","currency":"EUR"});
        assert_eq!(admit(&offer, &link(), "EUR", 5000).unwrap(), 5000);
        assert!(admit(&offer, &link(), "EUR", 4999).is_err());
        assert!(admit(&offer, &link(), "USD", 5000).is_err());
        for changed in [
            json!({"lineItems":[{"id":"lamp","quantity":11}],"totalMinor":"5000","currency":"EUR"}),
            json!({"lineItems":[{"id":"mug","quantity":10}],"totalMinor":"5000","currency":"EUR"}),
            json!({"lineItems":[{"id":"lamp","quantity":10},{"id":"mug","quantity":1}],"totalMinor":"5000","currency":"EUR"}),
            json!({"lineItems":[{"id":"lamp","quantity":10}],"totalMinor":50.0,"currency":"EUR"}),
        ] {
            assert!(admit(&changed, &link(), "EUR", 5000).is_err());
        }
    }

    #[test]
    fn a_proposal_without_ceiling_or_exact_total_is_not_approvable() {
        assert!(within_ceiling(true, Some(10), Some(10)));
        assert!(!within_ceiling(true, Some(11), Some(10)));
        assert!(!within_ceiling(false, Some(1), Some(10)));
        assert!(!within_ceiling(true, None, Some(10)));
        assert!(!within_ceiling(true, Some(1), None));
    }
}
