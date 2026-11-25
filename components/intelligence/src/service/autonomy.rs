//! Price-only optional autonomy, atomic unique-SKU daily quotas, a non-compounding day baseline and a daily exposure ceiling.
use crate::domain::price_guardrails::{Change, check};
use crate::store::autonomy as store;
use serde_json::{Value, json};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict};
use veyndo_identity::{allowed, permit};
use veyndo_kernel::verified_kernel;
use veyndo_pricing::catalog::Product;
use veyndo_pricing::service::conversion::amount;
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache::config;

/// The price changes an approved task asks to apply autonomously.
pub struct Autonomous<'a> {
    /// The task the changes belong to.
    pub task: &'a str,
    /// The proposed product changes.
    pub changes: &'a [Change],
    /// Whether the task also carries other work (an app action or an experience change).
    pub other_work: bool,
}
/// Reserves today's autonomy budget for price-only changes, or refuses them.
pub async fn reserve(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    h: &RequestContext,
    s: &Settings,
    p: Autonomous<'_>,
    products: &[Product],
) -> Result<()> {
    let id = p.task;
    permit(h, "catalog.write")?;
    permit(h, "settings.write")?;
    let used = store::used_today(&mut **tx, t).await?;
    let existing = store::products_today(&mut **tx, t).await?;
    let added = p
        .changes
        .iter()
        .filter(|c| !existing.contains(&c.product_id))
        .count();
    if !verified_kernel::ai_autonomy_admissible(
        s.ai_policy.autonomy.enabled,
        allowed(h, "catalog.write") && allowed(h, "settings.write"),
        !p.other_work
            && !p.changes.is_empty()
            && p.changes
                .iter()
                .all(|c| c.stock.is_none() && c.price.is_some()),
        used as usize + added <= s.ai_policy.autonomy.products_per_day as usize,
        true,
    ) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Autonomy is disabled, out of scope or exceeds today's unique-product budget".into(),
        ));
    }
    for c in p.changes {
        let product = products
            .iter()
            .find(|p| p.id == c.product_id)
            .ok_or(bad("Product unavailable"))?;
        check(s, product, c, true)?;
        let code = product.extra["priceCurrency"]
            .as_str()
            .unwrap_or(&s.currencies.pricing_currency);
        let scale = s
            .currencies
            .definitions
            .iter()
            .find(|d| d.code == code)
            .ok_or(bad("Currency missing"))?
            .scale;
        let first = store::reserve(
            &mut **tx,
            t,
            store::Baseline {
                product: &product.id,
                minor: amount(product.price, code, scale)?.minor(),
                currency: code,
                task: id,
                actor: header(h, "x-rac-user").unwrap_or("integration"),
            },
        )
        .await?;
        let proposed = amount(c.price.unwrap(), code, scale)?.minor();
        let baseline_currency = store::baseline_currency(&mut **tx, t, &product.id).await?;
        if code != baseline_currency
            || (first.abs_diff(proposed) as u128) * 10000
                > first as u128 * u128::from(s.ai_policy.autonomy.max_change_bps)
        {
            return Err(conflict(
                "Autonomy cannot compound beyond today's original price/currency budget",
            ));
        }
    }
    if let Some(limit) = s.ai_policy.autonomy.max_daily_exposure_minor {
        let mut proposed = Vec::new();
        for c in p.changes {
            let product = products
                .iter()
                .find(|p| p.id == c.product_id)
                .ok_or(bad("Product unavailable"))?;
            let code = product.extra["priceCurrency"]
                .as_str()
                .unwrap_or(&s.currencies.pricing_currency);
            if code != s.currencies.pricing_currency {
                return Err(conflict(
                    "The daily exposure limit only covers products priced in the shop's pricing currency",
                ));
            }
            proposed.push((
                product.id.clone(),
                minor(s, code, c.price.unwrap_or(product.price))?,
            ));
        }
        let (total, _) = today(&mut **tx, t, s, &proposed).await?;
        // The exposure ceiling joins the unique-product quota as the proved daily-budget conjunct.
        if !verified_kernel::ai_autonomy_admissible(
            s.ai_policy.autonomy.enabled,
            true,
            true,
            total <= u128::from(limit),
            true,
        ) {
            return Err(conflict(
                "Autonomy would exceed today's exposure limit for price reductions",
            ));
        }
    }
    Ok(())
}
/// Revenue given up if the whole current stock sold at the lower price; increases cost nothing here.
pub fn exposure(baseline_minor: i64, price_minor: i64, stock: i64) -> u128 {
    (baseline_minor.saturating_sub(price_minor).max(0) as u128) * (stock.max(0) as u128)
}
/// A price in minor units of its currency.
fn minor(s: &Settings, code: &str, price: f64) -> Result<i64> {
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == code)
        .ok_or(bad("Currency missing"))?
        .scale;
    Ok(amount(price, code, scale)?.minor())
}
/// Today's autonomous price rows with live price and stock; `proposed` replaces the live price of products about to change.
pub async fn today<'e>(
    db: impl sqlx::PgExecutor<'e>,
    t: &str,
    s: &Settings,
    proposed: &[(String, i64)],
) -> Result<(u128, Vec<Value>)> {
    let rows = store::exposures(db, t).await?;
    let (mut total, mut changes) = (0u128, Vec::new());
    for row in rows {
        let (id, code) = (row.product_id, row.currency);
        let (baseline, stock) = (row.baseline_minor, row.stock);
        let current = match proposed.iter().find(|(p, _)| p == &id) {
            Some((_, price)) => *price,
            None => minor(s, &code, row.price)?,
        };
        let counted = code == s.currencies.pricing_currency;
        let amount = exposure(baseline, current, i64::from(stock));
        if counted {
            total = total.saturating_add(amount);
        }
        changes.push(json!({"productId":id,"name":row.name,"currency":code,"baselineMinor":baseline,"currentMinor":current,"stock":stock,"exposureMinor":amount.min(u128::from(u64::MAX)) as u64,"counted":counted}));
    }
    Ok((total, changes))
}
/// Read model of the autopilot: level, limits and today's use. Reading reserves nothing.
pub async fn status(a: &State, t: &str, h: &RequestContext) -> Result<Value> {
    permit(h, "settings.read")?;
    let (s, revision) = config(a, t).await?;
    let policy = &s.ai_policy.autonomy;
    let (total, changes) = today(&a.db, t, &s, &[]).await?;
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == s.currencies.pricing_currency)
        .ok_or(bad("Currency missing"))?
        .scale;
    Ok(
        json!({"area":"prices","level":if policy.enabled {1} else {0},"enabled":policy.enabled,"settingsRevision":revision,
        "currency":s.currencies.pricing_currency,"scale":scale,"maxChangeBps":policy.max_change_bps,
        "productsPerDay":policy.products_per_day,"productsUsed":changes.len(),
        "maxDailyExposureMinor":policy.max_daily_exposure_minor,"exposureMinor":total.min(u128::from(u64::MAX)) as u64,
        "corridors":s.ai_policy.corridors.len(),"changes":changes}),
    )
}
#[cfg(test)]
mod tests {
    use super::exposure;
    #[test]
    fn exposure_counts_reductions_times_stock_and_never_increases_or_negative_stock() {
        assert_eq!(exposure(8000, 7600, 30), 12_000);
        assert_eq!(exposure(8000, 8400, 30), 0);
        assert_eq!(exposure(8000, 8000, 30), 0);
        assert_eq!(exposure(8000, 7600, 0), 0);
        assert_eq!(exposure(8000, 7600, -5), 0);
        assert_eq!(
            exposure(i64::MAX, 0, i64::MAX),
            (i64::MAX as u128) * (i64::MAX as u128)
        );
        assert_eq!(exposure(i64::MIN, i64::MAX, 10), 0);
    }
}
