//! Takes today's automatic price changes back to the day's baseline, by request or when the daily exposure limit is exceeded.
use super::autonomy::today;
use crate::knowledge;
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::atomic::{AtomicU64, Ordering};
use veyndo_catalog::store::products::product;
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
    let rows = sqlx::query("SELECT b.product_id,b.baseline_minor,b.currency,p.price,p.revision,k.proposal->'proposal'->'changes' AS changes FROM ai_price_budget b JOIN products p ON p.tenant=b.tenant AND p.id=b.product_id JOIN tasks k ON k.tenant=b.tenant AND k.id=b.task_id WHERE b.tenant=$1 AND b.day=(now() AT TIME ZONE 'UTC')::date AND ($2::text IS NULL OR b.product_id=$2) ORDER BY b.product_id FOR UPDATE OF p")
        .bind(t).bind(only).fetch_all(&mut *tx).await?;
    let (mut reverted, mut skipped) = (Vec::new(), Vec::new());
    for row in rows {
        let (id, code): (String, String) = (row.get("product_id"), row.get("currency"));
        let baseline: i64 = row.get("baseline_minor");
        let (current, scale) = minor(&s, &code, row.get("price"))?;
        if current == baseline {
            skipped.push(json!({"productId":id,"reason":"at_baseline"}));
            continue;
        }
        let set = row
            .get::<Option<Value>, _>("changes")
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
        let changed = sqlx::query("UPDATE products SET price=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4 RETURNING *")
            .bind(price).bind(t).bind(&id).bind(row.get::<i64,_>("revision")).fetch_optional(&mut *tx).await?;
        let Some(changed) = changed else {
            skipped.push(json!({"productId":id,"reason":"changed_since"}));
            continue;
        };
        knowledge::sync_product(&mut tx, t, &json!(product(&changed))).await?;
        veyndo_core::outbox::publish_on(
            &mut *tx,
            t,
            "product.updated",
            &json!({"productId":id,"revision":changed.get::<i64,_>("revision")}),
        )
        .await?;
        reverted.push(id);
    }
    if !reverted.is_empty() {
        veyndo_core::outbox::publish_on(
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
/// Takes back today's automatic changes on explicit request.
pub async fn request(a: &State, t: &str, h: &RequestContext, v: &Value) -> Result<Value> {
    permit(h, "catalog.write")?;
    permit(h, "settings.write")?;
    if v["approve"] != true {
        return Err(bad("Explicit approve=true required"));
    }
    revert(
        a,
        t,
        header(h, "x-rac-user").unwrap_or("integration"),
        v["productId"].as_str(),
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
    let tenants: Vec<String> = sqlx::query_scalar("SELECT DISTINCT tenant FROM ai_price_budget WHERE day=(now() AT TIME ZONE 'UTC')::date ORDER BY tenant LIMIT 500")
        .fetch_all(&a.db).await?;
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
