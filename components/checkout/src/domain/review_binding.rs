//! Bind the reviewed revision and exact total for every checkout transport.
use serde::Deserialize;
use serde_json::Value;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, bad, conflict};
use veyndo_pricing::verified_kernel;
use veyndo_shop::types::cart::StoredCart;
/// The JSON fields `validate` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ValidateInput {
    #[serde(deserialize_with = "lenient")]
    selection_needs_confirmation: Option<bool>,
}

pub fn validate(h: &RequestContext, c: &StoredCart, q: &Value, minor: i64) -> Result<()> {
    let input: ValidateInput = view(q);
    // Legacy API clients may omit the pair. Browser checkout always binds its reviewed quote.
    match (
        header(h, "x-commerce-cart-revision"),
        header(h, "x-commerce-total-minor"),
    ) {
        (None, None) => {}
        (Some(revision), Some(total)) => {
            let revision = revision
                .parse::<u64>()
                .map_err(|_| bad("Invalid reviewed cart revision"))?;
            let total = total
                .parse::<i64>()
                .map_err(|_| bad("Invalid reviewed total"))?;
            if !u64::try_from(total)
                .ok()
                .zip(u64::try_from(minor).ok())
                .is_some_and(|(expected, actual)| {
                    verified_kernel::checkout_review_admissible(
                        verified_kernel::revision_admissible(c.revision as u64, revision),
                        expected,
                        actual,
                        input.selection_needs_confirmation != Some(true),
                    )
                })
            {
                return Err(conflict("Checkout changed; review your order again"));
            }
        }
        _ => {
            return Err(bad(
                "Reviewed cart revision and total must be supplied together",
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::validate;
    use serde_json::json;
    use veyndo_core::RequestContext;
    use veyndo_shop::types::cart::StoredCart;

    fn cart(admin: Option<&str>) -> StoredCart {
        StoredCart {
            id: "cart-1".into(),
            tenant: "shop".into(),
            token: "token".into(),
            data: serde_json::from_value(json!({"items":[],"group":"retail","session":"s",
                "customer_id":"customer-1","locale":"de-DE","admin_actor":admin}))
            .expect("cart"),
            revision: 3,
            status: "open".into(),
        }
    }

    fn reviewed(revision: Option<&str>, total: Option<&str>) -> RequestContext {
        let mut h = RequestContext::new();
        if let Some(revision) = revision {
            h.insert(
                "x-commerce-cart-revision",
                revision.parse().expect("header"),
            );
        }
        if let Some(total) = total {
            h.insert("x-commerce-total-minor", total.parse().expect("header"));
        }
        h
    }

    #[test]
    fn the_order_is_bound_to_the_reviewed_revision_and_total() {
        let (c, q) = (cart(None), json!({}));
        assert!(validate(&reviewed(None, None), &c, &q, 1000).is_ok());
        assert!(validate(&reviewed(Some("3"), Some("1000")), &c, &q, 1000).is_ok());
        for (revision, total) in [("2", "1000"), ("3", "999"), ("x", "1000"), ("3", "x")] {
            assert!(
                validate(&reviewed(Some(revision), Some(total)), &c, &q, 1000).is_err(),
                "{revision} {total}"
            );
        }
        assert!(validate(&reviewed(Some("3"), None), &c, &q, 1000).is_err());
        assert!(validate(&reviewed(None, Some("1000")), &c, &q, 1000).is_err());
    }

    #[test]
    fn a_selection_that_needs_confirmation_or_a_negative_total_is_refused() {
        let c = cart(None);
        let h = reviewed(Some("3"), Some("1000"));
        assert!(validate(&h, &c, &json!({"selectionNeedsConfirmation":true}), 1000).is_err());
        assert!(validate(&reviewed(Some("3"), Some("-1")), &c, &json!({}), -1).is_err());
    }
}
