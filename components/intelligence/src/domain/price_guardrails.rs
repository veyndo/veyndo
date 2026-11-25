//! Merchant-owned AI price guardrails: corridors, margins, discounts and the autonomy limit, measured in native currency amounts.
use super::prompt_context::price_currency;
use serde::{Deserialize, Serialize};
use veyndo_core::{Result, bad, conflict};
use veyndo_kernel::verified_kernel;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion::amount;
use veyndo_shop::Settings;

/// One proposed product change: a new price, a new stock level, or both, bound to the revision it was planned on.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Change {
    pub product_id: String,
    #[serde(default)]
    pub expected_revision: i64,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub stock: Option<i32>,
}

/// Admits a proposed price change against the merchant corridor and, when autonomous, the autonomy policy.
pub fn check(s: &Settings, p: &Product, c: &Change, autonomous: bool) -> Result<()> {
    let Some(price) = c.price else {
        return Ok(());
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
    let proposed = amount(price, code, scale)?.minor() as u64;
    let before = amount(p.price, code, scale)?.minor() as u64;
    let rule = s.ai_policy.corridors.iter().find(|r| r.product_id == p.id);
    if autonomous && rule.is_none() {
        return Err(bad("Autonomy requires an explicit product corridor"));
    }
    if let Some(r) = rule {
        if r.currency != code {
            return Err(conflict(
                "AI corridor currency differs from product pricing currency",
            ));
        }
        // Base gross prices preserve the product's native base tax. This margin excludes shipping, fees and returns.
        let tax = (p.tax_rate * 100.).round();
        if !tax.is_finite() || !(0.0..=10000.0).contains(&tax) {
            return Err(bad("Invalid product tax basis"));
        }
        let net = (proposed as u128) * 10000 / (10000 + tax as u128);
        let margin = r.unit_cost_net_minor.is_none_or(|cost| {
            net * u128::from(10000 - r.minimum_margin_bps) >= u128::from(cost) * 10000
        });
        let discount = r.maximum_discount_bps.is_none_or(|limit| {
            (proposed as u128) * 10000 >= (before as u128) * u128::from(10000 - limit)
        });
        if !verified_kernel::ai_price_admissible(
            proposed,
            r.minimum_minor,
            r.maximum_minor,
            margin,
            discount,
            !r.price_locked || proposed == before,
            !r.require_available || p.stock > 0,
        ) {
            return Err(conflict(
                "AI proposal violates current price, margin, discount, brand or availability guardrails",
            ));
        }
    }
    if autonomous {
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
    use super::{Change, check};
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
            price,
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
}
