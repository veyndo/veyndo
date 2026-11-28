//! Merchant-owned AI price guardrails: corridors, margins, discounts and the autonomy limit, measured in native currency amounts.
use super::prompt_context::price_currency;
use serde::{Deserialize, Serialize};
use veyndo_core::{Result, bad, conflict};
use veyndo_kernel::verified_kernel;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::decimal::{Decimal, Ratio};
use veyndo_pricing::service::conversion::exact_amount;
use veyndo_shop::Settings;
use veyndo_shop::types::guardrails::Corridor;

/// One proposed product change: a new price, a new stock level, or both, bound to the revision it was planned on.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Change {
    pub product_id: String,
    #[serde(default)]
    pub expected_revision: i64,
    /// The content clock a price-only change was planned on; native admission sets it, never the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_content_revision: Option<i64>,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub stock: Option<i32>,
}

/// The individual guardrail answers for one proposed price, in the product's native currency minor units.
pub struct Assessment<'a> {
    /// The proposed and current price in minor units.
    pub proposed: u64,
    pub before: u64,
    /// The currency code and scale the amounts use.
    pub currency: String,
    pub scale: u32,
    /// The merchant corridor of the product, if one is configured.
    pub corridor: Option<&'a Corridor>,
    /// The corridor's currency differs from the product pricing currency.
    pub currency_mismatch: bool,
    pub margin: bool,
    pub discount: bool,
    pub unlocked: bool,
    pub available: bool,
}

/// Measures a proposed price against the corridor; `None` when the change sets no price.
pub fn assess<'a>(s: &'a Settings, p: &Product, c: &Change) -> Result<Option<Assessment<'a>>> {
    let Some(price) = c.price else {
        return Ok(None);
    };
    let own = price_currency(&p.extra);
    let code = own.as_deref().unwrap_or(&s.currencies.pricing_currency);
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == code)
        .ok_or(bad("Product price currency is not configured"))?
        .scale;
    let proposed = exact_amount(price, code, scale)?.minor() as u64;
    let before = exact_amount(p.price, code, scale)?.minor() as u64;
    let corridor = s.ai_policy.corridors.iter().find(|r| r.product_id == p.id);
    let mut assessment = Assessment {
        proposed,
        before,
        currency: code.to_string(),
        scale: u32::from(scale),
        corridor,
        currency_mismatch: false,
        margin: true,
        discount: true,
        unlocked: true,
        available: true,
    };
    let Some(r) = corridor else {
        return Ok(Some(assessment));
    };
    if r.currency != code {
        assessment.currency_mismatch = true;
        return Ok(Some(assessment));
    }
    // Base gross prices preserve the product's native base tax. This margin excludes shipping, fees and returns.
    // The rate in basis points, rounded half away from zero.
    let tax = tax_bps(p)?;
    let net = (proposed as u128) * 10000 / (10000 + tax as u128);
    assessment.margin = r.unit_cost_net_minor.is_none_or(|cost| {
        net * u128::from(10000 - r.minimum_margin_bps) >= u128::from(cost) * 10000
    });
    assessment.discount = r.maximum_discount_bps.is_none_or(|limit| {
        (proposed as u128) * 10000 >= (before as u128) * u128::from(10000 - limit)
    });
    assessment.unlocked = !r.price_locked || proposed == before;
    assessment.available = !r.require_available || p.stock > 0;
    Ok(Some(assessment))
}

/// The product's base tax rate in basis points, rounded half away from zero.
pub fn tax_bps(p: &Product) -> Result<i64> {
    let tax = p
        .tax_rate
        .ratio()
        .checked_mul(Ratio::integer(100))
        .and_then(|bps| bps.rounded(0))
        .map_err(|_| bad("Invalid product tax basis"))?
        .minor();
    if !(0..=10000).contains(&tax) {
        return Err(bad("Invalid product tax basis"));
    }
    Ok(tax)
}

/// The guardrails a proposed price violates, as stable codes; empty exactly when [`check`] admits it for review.
pub fn violations(s: &Settings, p: &Product, c: &Change) -> Result<Vec<&'static str>> {
    let Some(a) = assess(s, p, c)? else {
        return Ok(vec![]);
    };
    let Some(r) = a.corridor else {
        return Ok(vec![]);
    };
    if a.currency_mismatch {
        return Ok(vec!["corridorCurrency"]);
    }
    let found = [
        (a.proposed < r.minimum_minor, "corridorMinimum"),
        (a.proposed > r.maximum_minor, "corridorMaximum"),
        (!a.margin, "minimumMargin"),
        (!a.discount, "maximumDiscount"),
        (!a.unlocked, "priceLocked"),
        (!a.available, "requiresAvailability"),
    ];
    Ok(found
        .into_iter()
        .filter_map(|(violated, code)| violated.then_some(code))
        .collect())
}

/// Admits a proposed price change against the merchant corridor and, when autonomous, the autonomy policy.
pub fn check(s: &Settings, p: &Product, c: &Change, autonomous: bool) -> Result<()> {
    let Some(a) = assess(s, p, c)? else {
        return Ok(());
    };
    if autonomous && a.corridor.is_none() {
        return Err(bad("Autonomy requires an explicit product corridor"));
    }
    if let Some(r) = a.corridor {
        if a.currency_mismatch {
            return Err(conflict(
                "AI corridor currency differs from product pricing currency",
            ));
        }
        if !verified_kernel::ai_price_admissible(
            a.proposed,
            r.minimum_minor,
            r.maximum_minor,
            a.margin,
            a.discount,
            a.unlocked,
            a.available,
        ) {
            return Err(conflict(
                "AI proposal violates current price, margin, discount, brand or availability guardrails",
            ));
        }
    }
    if autonomous {
        let (before, proposed) = (a.before, a.proposed);
        let delta = before.abs_diff(proposed) as u128;
        if !verified_kernel::ai_autonomy_admissible(
            s.ai_policy.autonomy.enabled,
            true,
            c.stock.is_none(),
            true,
            delta * 10000 <= before as u128 * u128::from(s.ai_policy.autonomy.max_change_bps),
        ) {
            return Err(conflict("Autonomous price change exceeds current policy"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Change, Decimal, check, violations};
    use serde_json::json;
    use veyndo_pricing::catalog::Product;
    use veyndo_shop::Settings;

    fn product(price: f64, stock: i32) -> Product {
        serde_json::from_value(json!({
            "extra":{},"id":"p1","name":"Lamp","category":"","description":"","price":price,
            "tax_rate":19.0,"stock":stock,"revision":1,"list_price":null,"regulation_price":null,
            "reference_price":null,"advanced_prices":[],"min_purchase":1,"purchase_steps":1,
            "max_purchase":null,"parent_id":null,"options":{},"media":[],"properties":{},
            "delivery_days":1
        }))
        .unwrap()
    }

    fn settings(corridor: Option<serde_json::Value>, autonomy: bool) -> Settings {
        let mut s: Settings =
            serde_json::from_str(include_str!("../../../../fixtures/demo-settings.json")).unwrap();
        let code = s.currencies.pricing_currency.clone();
        let mut policy = json!({"corridors":[],"autonomy":{"enabled":autonomy,"maxChangeBps":500,"productsPerDay":5}});
        if let Some(mut c) = corridor {
            c["currency"] = json!(code);
            policy["corridors"] = json!([c]);
        }
        s.ai_policy = serde_json::from_value(policy).unwrap();
        s
    }

    fn change(price: Option<f64>) -> Change {
        Change {
            product_id: "p1".into(),
            expected_revision: 1,
            expected_content_revision: None,
            price: price.map(|p| Decimal::from_wire(p).unwrap()),
            stock: None,
        }
    }

    #[test]
    fn a_change_without_price_needs_no_corridor() {
        assert!(
            check(
                &settings(None, false),
                &product(100., 1),
                &change(None),
                true
            )
            .is_ok()
        );
    }

    #[test]
    fn prices_stay_inside_the_corridor_and_autonomy_needs_one() {
        let corridor = json!({"productId":"p1","minimumMinor":9000,"maximumMinor":11000});
        let s = settings(Some(corridor), true);
        let p = product(100., 1);
        assert!(check(&s, &p, &change(Some(104.)), false).is_ok());
        assert!(check(&s, &p, &change(Some(120.)), false).is_err());
        assert!(check(&s, &p, &change(Some(104.)), true).is_ok());
        assert!(check(&s, &p, &change(Some(108.)), true).is_err());
        let open = settings(None, true);
        assert!(check(&open, &p, &change(Some(101.)), false).is_ok());
        assert!(check(&open, &p, &change(Some(101.)), true).is_err());
    }

    #[test]
    fn locked_and_unavailable_products_keep_their_price() {
        let locked =
            json!({"productId":"p1","minimumMinor":1,"maximumMinor":100000,"priceLocked":true});
        let s = settings(Some(locked), false);
        assert!(check(&s, &product(100., 1), &change(Some(100.)), false).is_ok());
        assert!(check(&s, &product(100., 1), &change(Some(99.)), false).is_err());
        let available = json!({"productId":"p1","minimumMinor":1,"maximumMinor":100000,"requireAvailable":true});
        let s = settings(Some(available), false);
        assert!(check(&s, &product(100., 0), &change(Some(99.)), false).is_err());
    }

    #[test]
    fn violations_name_exactly_the_guardrails_check_refuses() {
        let corridor = json!({"productId":"p1","minimumMinor":9000,"maximumMinor":11000,
            "minimumMarginBps":2000,"unitCostNetMinor":6000,"maximumDiscountBps":500,"requireAvailable":true});
        let s = settings(Some(corridor), false);
        for (price, stock, expected) in [
            (100., 1, vec![]),
            (120., 1, vec!["corridorMaximum"]),
            (
                80.,
                1,
                vec!["corridorMinimum", "minimumMargin", "maximumDiscount"],
            ),
            (100., 0, vec!["requiresAvailability"]),
        ] {
            let p = product(100., stock);
            let found = violations(&s, &p, &change(Some(price))).unwrap();
            assert_eq!(found, expected, "price {price} stock {stock}");
            assert_eq!(
                check(&s, &p, &change(Some(price)), false).is_ok(),
                found.is_empty()
            );
        }
        let locked =
            json!({"productId":"p1","minimumMinor":1,"maximumMinor":100000,"priceLocked":true});
        let s = settings(Some(locked), false);
        assert_eq!(
            violations(&s, &product(100., 1), &change(Some(99.))).unwrap(),
            vec!["priceLocked"]
        );
        assert!(
            violations(&settings(None, false), &product(100., 1), &change(Some(1.)))
                .unwrap()
                .is_empty()
        );
    }
}
