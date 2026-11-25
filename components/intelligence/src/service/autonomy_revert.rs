//! Takes today's automatic price changes back to the day's baseline, by request or when the daily exposure limit is exceeded.
use super::autonomy::today;
use crate::knowledge;
use crate::store::autonomy as store;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::header;
use veyndo_core::{Error, RequestContext, Result, State, bad};
use veyndo_identity::permit;
use veyndo_pricing::money::{Currency, Money};
use veyndo_pricing::service::conversion::amount;
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache::config;
static LAST_GUARD: AtomicU64 = AtomicU64::new(0);
/// A price in minor units with its scale.
fn minor(s: &Settings, code: &str, price: f64) -> Result<(i64, u8)> {
    let scale = s
        .currencies
        .definitions
        .iter()
        .find(|d| d.code == code)
        .ok_or(bad("Currency missing"))?
        .scale;
    Ok((amount(price, code, scale)?.minor(), scale))
}
/// A product is only restored while it still carries the price the autopilot set; a later manual edit wins.
pub async fn revert(
    a: &State,
    t: &str,
    actor: &str,
    only: Option<&str>,
    automatic: bool,
) -> Result<Value> {
    let (s, _) = config(a, t).await?;
    let mut tx = a.db.begin().await?;
    let rows = store::changed_locked(&mut *tx, t, only).await?;
    let (mut reverted, mut skipped) = (Vec::new(), Vec::new());
    for row in rows {
        let (id, code) = (row.product_id, row.currency);
        let baseline = row.baseline_minor;
        let (current, scale) = minor(&s, &code, row.price)?;
        if current == baseline {
            skipped.push(json!({"productId":id,"reason":"at_baseline"}));
            continue;
        }
        let set = row
            .changes
            .and_then(|changes| {
                changes.as_array().and_then(|list| {
                    list.iter()
                        .find(|c| c["product_id"].as_str() == Some(id.as_str()))
                        .and_then(|c| c["price"].as_f64())
                })
            })
            .and_then(|price| minor(&s, &code, price).ok())
            .map(|(value, _)| value);
        if set != Some(current) {
            skipped.push(json!({"productId":id,"reason":"changed_since"}));
            continue;
        }
        let price = Money::new(baseline, Currency::new(&code, scale).map_err(bad)?)
            .decimal()
            .parse::<f64>()
            .map_err(|_| bad("Invalid baseline price"))?;
        let changed = store::restore_price(&mut tx, t, &id, price, row.revision).await?;
        let Some(changed) = changed else {
            skipped.push(json!({"productId":id,"reason":"changed_since"}));
            continue;
        };
        knowledge::sync_product(&mut tx, t, &json!(changed)).await?;
        veyndo_core::store::outbox::publish_on(
            &mut *tx,
            t,
            "product.updated",
            &json!({"productId":id,"revision":changed.revision}),
        )
        .await?;
        reverted.push(id);
    }
    if !reverted.is_empty() {
        veyndo_core::store::outbox::publish_on(
            &mut *tx,
            t,
            "ai.autonomy.reverted",
            &json!({"actor":actor,"automatic":automatic,"productIds":reverted}),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(json!({"reverted":reverted,"skipped":skipped,"automatic":automatic}))
}
/// The JSON fields `request` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RequestInput {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
}

/// Takes back today's automatic changes on explicit request.
pub async fn request(a: &State, t: &str, h: &RequestContext, v: &Value) -> Result<Value> {
    let input: RequestInput = view(v);
    permit(h, "catalog.write")?;
    permit(h, "settings.write")?;
    if input.approve != Some(true) {
        return Err(bad("Explicit approve=true required"));
    }
    revert(
        a,
        t,
        header(h, "x-rac-user").unwrap_or("integration"),
        input.product_id.as_deref(),
        false,
    )
    .await
}
/// Worker guard: at most every 30 seconds per process, restore shops whose exposure today is above their limit.
pub async fn guard_once(a: &State) -> Result<bool> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let last = LAST_GUARD.load(Ordering::Relaxed);
    if now < last + 30
        || LAST_GUARD
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return Ok(false);
    }
    let tenants = store::shops_today(&a.db).await?;
    let mut worked = false;
    for t in tenants {
        // One shop's failure must not stop the guard for the others.
        let result = async {
            let (s, _) = config(a, &t).await?;
            let Some(limit) = s.ai_policy.autonomy.max_daily_exposure_minor else {
                return Ok(false);
            };
            let (total, _) = today(&a.db, &t, &s, &[]).await?;
            if total <= u128::from(limit) {
                return Ok(false);
            }
            let outcome = revert(a, &t, "autopilot-guard", None, true).await?;
            Ok::<_, Error>(
                outcome["reverted"]
                    .as_array()
                    .is_some_and(|r| !r.is_empty()),
            )
        }
        .await;
        match result {
            Ok(done) => worked |= done,
            Err(e) => tracing::warn!(tenant = %t, status = e.0.as_u16(), "autopilot guard failed"),
        }
    }
    Ok(worked)
}
