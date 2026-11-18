//! Price-only optional autonomy, atomic unique-SKU daily quotas, a non-compounding day baseline and a daily exposure ceiling.
use super::*;
pub(crate) async fn reserve(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    h: &RequestContext,
    s: &commerce::Settings,
    p: &Proposal,
    products: &[Product],
) -> Result<()> {
    auth::permit(h, "catalog.write")?;
    auth::permit(h, "settings.write")?;
    let used:i64=sqlx::query_scalar("SELECT count(*) FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date").bind(t).fetch_one(&mut **tx).await?;
    let existing:Vec<String>=sqlx::query_scalar("SELECT product_id FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date").bind(t).fetch_all(&mut **tx).await?;
    let added = p
        .changes
        .iter()
        .filter(|c| !existing.contains(&c.product_id))
        .count();
    if !verified_kernel::ai_autonomy_admissible(
        s.ai_policy.autonomy.enabled,
        auth::allowed(h, "catalog.write") && auth::allowed(h, "settings.write"),
        p.app_action.is_none()
            && p.experience.is_none()
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
    for c in &p.changes {
        let product = products
            .iter()
            .find(|p| p.id == c.product_id)
            .ok_or(bad("Product unavailable"))?;
        guardrails::check(s, product, c, true)?;
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
        let first:i64=sqlx::query_scalar("INSERT INTO ai_price_budget(tenant,day,product_id,baseline_minor,currency,task_id,actor) VALUES($1,(now() AT TIME ZONE 'UTC')::date,$2,$3,$4,$5,$6) ON CONFLICT(tenant,day,product_id) DO UPDATE SET task_id=EXCLUDED.task_id RETURNING baseline_minor")
            .bind(t).bind(&product.id).bind(currencies::amount(product.price,code,scale)?.minor()).bind(code).bind(id).bind(header(h,"x-rac-user").unwrap_or("integration")).fetch_one(&mut **tx).await?;
        let proposed = currencies::amount(c.price.unwrap(), code, scale)?.minor();
        let baseline_currency:String=sqlx::query_scalar("SELECT currency FROM ai_price_budget WHERE tenant=$1 AND day=(now() AT TIME ZONE 'UTC')::date AND product_id=$2").bind(t).bind(&product.id).fetch_one(&mut **tx).await?;
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
        for c in &p.changes {
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
pub(crate) fn exposure(baseline_minor: i64, price_minor: i64, stock: i64) -> u128 {
    (baseline_minor.saturating_sub(price_minor).max(0) as u128) * (stock.max(0) as u128)
}
fn minor(s: &commerce::Settings, code: &str, price: f64) -> Result<i64> {
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == code)
        .ok_or(bad("Currency missing"))?
        .scale;
    Ok(currencies::amount(price, code, scale)?.minor())
}
/// Today's autonomous price rows with live price and stock; `proposed` replaces the live price of products about to change.
pub(crate) async fn today<'e>(
    db: impl sqlx::PgExecutor<'e>,
    t: &str,
    s: &commerce::Settings,
    proposed: &[(String, i64)],
) -> Result<(u128, Vec<Value>)> {
    let rows = sqlx::query("SELECT b.product_id,b.baseline_minor,b.currency,p.name,p.price,p.stock FROM ai_price_budget b JOIN products p ON p.tenant=b.tenant AND p.id=b.product_id WHERE b.tenant=$1 AND b.day=(now() AT TIME ZONE 'UTC')::date ORDER BY b.product_id")
        .bind(t).fetch_all(db).await?;
    let (mut total, mut changes) = (0u128, Vec::new());
    for row in rows {
        let (id, code): (String, String) = (row.get("product_id"), row.get("currency"));
        let (baseline, stock): (i64, i32) = (row.get("baseline_minor"), row.get("stock"));
        let current = match proposed.iter().find(|(p, _)| p == &id) {
            Some((_, price)) => *price,
            None => minor(s, &code, row.get("price"))?,
        };
        let counted = code == s.currencies.pricing_currency;
        let amount = exposure(baseline, current, i64::from(stock));
        if counted {
            total = total.saturating_add(amount);
        }
        changes.push(json!({"productId":id,"name":row.get::<String,_>("name"),"currency":code,"baselineMinor":baseline,"currentMinor":current,"stock":stock,"exposureMinor":amount.min(u128::from(u64::MAX)) as u64,"counted":counted}));
    }
    Ok((total, changes))
}
/// Read model of the autopilot: level, limits and today's use. Reading reserves nothing.
pub(crate) async fn status(a: &App, t: &str, h: &RequestContext) -> Result<Value> {
    auth::permit(h, "settings.read")?;
    let (s, revision) = commerce::config(a, t).await?;
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
