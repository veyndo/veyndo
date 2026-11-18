//! Experiment suggestions from native figures, and worker steps that finish, read out and apply an approved experiment without further clicks.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static LAST: AtomicU64 = AtomicU64::new(0);
const ACTOR: &str = "experiment-autopilot";
/// A prefilled preregistration with the figures it is based on. Nothing is saved and no model is called.
pub(super) async fn suggest(a: &App, h: &RequestContext, t: &str) -> Result<Value> {
    let (settings, _) = commerce::config(a, t).await?;
    let currency = if settings
        .currencies
        .enabled
        .contains(&settings.currencies.pricing_currency)
    {
        settings.currencies.pricing_currency.clone()
    } else {
        settings
            .currencies
            .enabled
            .first()
            .cloned()
            .ok_or(bad("Enable a currency before planning an experiment"))?
    };
    let channel = marketing::channel_id(h);
    let row = sqlx::query("SELECT count(*) AS payments,coalesce(percentile_cont(0.95) WITHIN GROUP (ORDER BY p.amount_minor),0)::bigint AS p95 FROM payment_attempts p WHERE p.tenant=$1 AND p.currency=$2 AND p.environment='live' AND p.state IN ('captured','captured_late') AND p.capture_confirmed_at>now()-interval '30 days'")
        .bind(t).bind(&currency).fetch_one(&a.db).await?;
    let (payments, p95): (i64, i64) = (row.get("payments"), row.get("p95"));
    let mode: String = sqlx::query_scalar(
        "SELECT coalesce(data->>'mode','balanced') FROM experiences WHERE tenant=$1",
    )
    .bind(t)
    .fetch_optional(&a.db)
    .await?
    .unwrap_or_else(|| "balanced".into());
    let running: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running')")
        .bind(t).bind(channel).fetch_one(&a.db).await?;
    let control = if mode == "comparison" {
        "comparison"
    } else {
        "discovery"
    };
    let treatment = if control == "discovery" {
        "comparison"
    } else {
        "discovery"
    };
    // Both arms need the minimum; with fewer paid orders a month the readout would stay empty.
    let warnings: Vec<&str> = [
        (payments == 0, "no_live_payments"),
        (payments > 0 && payments < 200, "low_volume"),
        (running, "already_running"),
    ]
    .into_iter()
    .filter_map(|(hit, code)| hit.then_some(code))
    .collect();
    let design = model::Design {
        title: format!("{control} vs {treatment}"),
        locale: settings.main_locale.clone(),
        channel: channel.to_owned(),
        control: control.into(),
        treatment: treatment.into(),
        currency: currency.clone(),
        duration_hours: if payments >= 400 { 168 } else { 336 },
        settlement_days: 14,
        minimum_per_arm: 100,
        outcome_cap_minor: p95.clamp(1_000, 1_000_000_000_000),
        cuped_theta: 0.0,
        automation: model::Automation {
            finish: true,
            apply: false,
        },
    };
    design.validate()?;
    Ok(
        json!({"design":design,"basis":{"scope":"shop","livePayments30Days":payments,"p95PaymentMinor":p95,"currentMode":mode,"currency":currency,"scale":currencies::scale(&currency)},"warnings":warnings,"saved":false}),
    )
}
/// Worker lane step, at most every 30 seconds per process. Every write repeats the manual transition's own state check.
pub(crate) async fn auto_once(a: &App) -> Result<bool> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let last = LAST.load(Ordering::Relaxed);
    if now < last + 30
        || LAST
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return Ok(false);
    }
    let mut worked = false;
    let due = sqlx::query("UPDATE intelligence_experiments SET state='completed',revision=revision+1 WHERE state='running' AND ends_at<=now() AND coalesce((data->'automation'->>'finish')::boolean,false) RETURNING tenant,id")
        .fetch_all(&a.db).await?;
    for row in due {
        worked = true;
        sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'intelligence.experiment.changed',$2)")
            .bind(row.get::<String,_>("tenant"))
            .bind(json!({"id":row.get::<String,_>("id"),"operation":"knowledge.experiment.finish","actor":ACTOR}))
            .execute(&a.db).await?;
    }
    let settled = sqlx::query("SELECT tenant,id,data FROM intelligence_experiments WHERE state='completed' AND withdrawn=0 AND coalesce((data->'automation'->>'finish')::boolean,false) AND (final_report IS NULL OR NOT final_report ? 'decision') AND now()>=ends_at+((data->>'settlementDays')::integer*interval '1 day') ORDER BY ends_at LIMIT 50")
        .fetch_all(&a.db).await?;
    for row in settled {
        let (t, id): (String, String) = (row.get("tenant"), row.get("id"));
        let outcome = async {
            let design: model::Design = serde_json::from_value(row.get("data"))
                .map_err(|_| bad("Invalid stored experiment"))?;
            let result = report::report(a, &t, &id).await?;
            if result["finalLook"] != true {
                return Ok(false);
            }
            let publish = design.automation.apply && result["causalUpliftProven"] == true;
            let mut tx = a.db.begin().await?;
            let decided = sqlx::query("UPDATE intelligence_experiments SET final_report=final_report||$3 WHERE tenant=$1 AND id=$2 AND final_report IS NOT NULL AND NOT final_report ? 'decision'")
                .bind(&t).bind(&id)
                .bind(json!({"decision":if publish {"published"} else {"kept"},"decidedAt":chrono::Utc::now().to_rfc3339(),"decidedBy":ACTOR}))
                .execute(&mut *tx).await?.rows_affected();
            if decided != 1 {
                return Ok(false);
            }
            if publish {
                // The proved layout becomes the shop's fixed layout; without proof the shop stays as it is.
                sqlx::query("UPDATE experiences SET data=jsonb_set(data,'{mode}',to_jsonb($2::text)),revision=revision+1 WHERE tenant=$1")
                    .bind(&t).bind(&design.treatment).execute(&mut *tx).await?;
            }
            sqlx::query("INSERT INTO outbox(tenant,kind,data) VALUES($1,'intelligence.experiment.changed',$2)")
                .bind(&t)
                .bind(json!({"id":id,"operation":"knowledge.experiment.decide","decision":if publish {"published"} else {"kept"},"actor":ACTOR}))
                .execute(&mut *tx).await?;
            tx.commit().await?;
            Ok::<_, Error>(true)
        }
        .await;
        match outcome {
            Ok(done) => worked |= done,
            Err(e) => {
                tracing::warn!(tenant = %t, status = e.0.as_u16(), "experiment automation failed")
            }
        }
    }
    Ok(worked)
}
