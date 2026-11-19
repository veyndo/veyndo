//! Merchant-owned guardrails in commerce settings; native currency amounts bind preview and execution.
use super::*;
pub(crate) fn check(
    s: &commerce::Settings,
    p: &Product,
    c: &Change,
    autonomous: bool,
) -> Result<()> {
    let Some(price) = c.price else {
        return Ok(());
    };
    let code = p.extra["priceCurrency"]
        .as_str()
        .unwrap_or(&s.currencies.pricing_currency);
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == code)
        .ok_or(bad("Product price currency is not configured"))?
        .scale;
    let proposed = currencies::amount(price, code, scale)?.minor() as u64;
    let before = currencies::amount(p.price, code, scale)?.minor() as u64;
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
pub(crate) async fn validate_plan(a: &App, t: &str, p: &Proposal, ps: &[Product]) -> Result<i64> {
    let (settings, revision) = commerce::config(a, t).await?;
    for c in &p.changes {
        check(
            &settings,
            ps.iter()
                .find(|p| p.id == c.product_id)
                .ok_or(bad("Unknown product"))?,
            c,
            false,
        )?;
    }
    Ok(revision)
}
