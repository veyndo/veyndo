//! Controlled experiments on the storefront layout: preregistration, transitions, assignment, the delayed readout from the payment ledger and the worker steps.
use crate::domain::experiment_model::{Automation, Design, interval};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::sync::atomic::{AtomicU64, Ordering};
use veyndo_core::{Error, Result, State, bad, conflict, hash};
use veyndo_pricing::types::currency::scale;

static LAST: AtomicU64 = AtomicU64::new(0);
const ACTOR: &str = "experiment-autopilot";

/// The registered experiments of a shop, newest first.
pub async fn list(a: &State, t: &str) -> Result<Value> {
    let rows=sqlx::query("SELECT id,data,revision,state,started_at::text,ends_at::text FROM intelligence_experiments WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100").bind(t).fetch_all(&a.db).await?;
    Ok(
        json!({"experiments":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"design":r.get::<Value,_>("data"),"revision":r.get::<i64,_>("revision"),"state":r.get::<String,_>("state"),"startedAt":r.get::<Option<String>,_>("started_at"),"endsAt":r.get::<Option<String>,_>("ends_at")})).collect::<Vec<_>>() }),
    )
}

/// Registers a design or moves an experiment through its fixed transitions at the expected revision.
pub async fn change(
    a: &State,
    t: &str,
    name: &str,
    v: &Value,
    actor: &str,
    currencies: &[String],
) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,24))")
        .bind(t)
        .execute(&mut *tx)
        .await?;
    let id = if name == "knowledge.experiment.create" {
        let design: Design = serde_json::from_value(v["design"].clone())
            .map_err(|_| bad("Invalid experiment design"))?;
        design.validate()?;
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM intelligence_experiments WHERE tenant=$1")
                .bind(t)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 100 {
            return Err(bad("Experiment history limit reached"));
        }
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sales_channels WHERE tenant=$1 AND id=$2)",
        )
        .bind(t)
        .bind(&design.channel)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(bad("Channel does not belong to shop"));
        }
        if !currencies.contains(&design.currency) {
            return Err(bad("Experiment currency is not enabled"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO intelligence_experiments(tenant,id,channel,data,state,actor) VALUES($1,$2,$3,$4,'draft',$5)").bind(t).bind(&id).bind(&design.channel).bind(json!(design)).bind(actor).execute(&mut *tx).await?;
        id
    } else {
        let id = v["id"].as_str().ok_or(bad("Experiment required"))?;
        let r=sqlx::query("SELECT data,revision,state FROM intelligence_experiments WHERE tenant=$1 AND id=$2 FOR UPDATE").bind(t).bind(id).fetch_optional(&mut *tx).await?.ok_or(bad("Experiment not found"))?;
        if v["revision"].as_i64() != Some(r.get("revision")) {
            return Err(conflict("Experiment revision changed"));
        }
        let state: String = r.get("state");
        if name == "knowledge.experiment.start" && state == "draft" {
            let d: Design = serde_json::from_value(r.get("data"))
                .map_err(|_| bad("Invalid stored experiment"))?;
            d.validate()?;
            let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running')").bind(t).bind(&d.channel).fetch_one(&mut *tx).await?;
            if active {
                return Err(conflict("A channel experiment is already running"));
            }
            sqlx::query("UPDATE intelligence_experiments SET state='running',started_at=now(),ends_at=now()+($3*interval '1 hour'),revision=revision+1 WHERE tenant=$1 AND id=$2").bind(t).bind(id).bind(d.duration_hours as f64).execute(&mut *tx).await?;
        } else if name == "knowledge.experiment.finish" && state == "running" {
            let elapsed: bool = sqlx::query_scalar(
                "SELECT ends_at<=now() FROM intelligence_experiments WHERE tenant=$1 AND id=$2",
            )
            .bind(t)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if !elapsed {
                return Err(conflict("Preregistered horizon has not elapsed"));
            }
            sqlx::query("UPDATE intelligence_experiments SET state='completed',revision=revision+1 WHERE tenant=$1 AND id=$2").bind(t).bind(id).execute(&mut *tx).await?;
        } else if name == "knowledge.experiment.stop" && state == "running" {
            sqlx::query("UPDATE intelligence_experiments SET state='stopped',revision=revision+1 WHERE tenant=$1 AND id=$2").bind(t).bind(id).execute(&mut *tx).await?;
            sqlx::query("UPDATE knowledge_relations SET data=jsonb_set(jsonb_set(jsonb_set(data,'{causalUpliftProven}','false'),'{interval95}','null'),'{stoppedEarly}','true') WHERE tenant=$1 AND kind='EXPERIMENT_RESULT' AND source_id=$2").bind(t).bind(id).execute(&mut *tx).await?;
        } else {
            return Err(bad(
                "Invalid experiment transition; design is immutable after registration",
            ));
        }
        id.to_owned()
    };
    veyndo_core::outbox::publish_on(
        &mut *tx,
        t,
        "intelligence.experiment.changed",
        &json!({"id":id,"operation":name,"actor":actor}),
    )
    .await?;
    tx.commit().await?;
    Ok(json!({"id":id,"saved":true}))
}
/// The cart an assignment is made for.
pub struct Subject<'a> {
    pub tenant: &'a str,
    pub cart: &'a str,
    pub channel: &'a str,
    pub open: bool,
    pub currency: &'a str,
}

/// Called only inside the native consent-locked experience transaction. Never overrides non-consenting layouts.
pub async fn assign(
    tx: &mut Transaction<'_, Postgres>,
    secret: &str,
    s: Subject<'_>,
) -> Result<Option<Value>> {
    let r=sqlx::query("SELECT id,data FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running' AND ends_at>now() FOR UPDATE").bind(s.tenant).bind(s.channel).fetch_optional(&mut **tx).await?;
    let Some(r) = r else { return Ok(None) };
    let id: String = r.get("id");
    let d: Design =
        serde_json::from_value(r.get("data")).map_err(|_| bad("Invalid stored experiment"))?;
    if !s.open || s.currency != d.currency {
        return Ok(None);
    };
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM intelligence_assignments WHERE tenant=$1 AND experiment_id=$2",
    )
    .bind(s.tenant)
    .bind(&id)
    .fetch_one(&mut **tx)
    .await?;
    let saved:Option<i32>=sqlx::query_scalar("SELECT arm FROM intelligence_assignments WHERE tenant=$1 AND experiment_id=$2 AND cart_id=$3").bind(s.tenant).bind(&id).bind(s.cart).fetch_optional(&mut **tx).await?;
    let arm = if let Some(arm) = saved {
        arm
    } else {
        if total >= 10000 {
            return Ok(None);
        };
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        let arm = i32::from(
            ring::hmac::sign(&key, format!("{}:{}:{}", s.tenant, id, s.cart).as_bytes()).as_ref()
                [0]
                % 2,
        );
        let baseline:i64=sqlx::query_scalar("SELECT coalesce(sum(views),0)::bigint FROM session_signals WHERE tenant=$1 AND session=$2").bind(s.tenant).bind(hash(s.cart)).fetch_one(&mut **tx).await?;
        sqlx::query("INSERT INTO intelligence_assignments(tenant,experiment_id,cart_id,arm,baseline) VALUES($1,$2,$3,$4,$5)").bind(s.tenant).bind(&id).bind(s.cart).bind(arm).bind(baseline.clamp(0,1000) as f64/1000.0).execute(&mut **tx).await?;
        arm
    };
    Ok(Some(
        json!({"variant":if arm==0{d.control}else{d.treatment},"propensity":0.5,"experimentId":id,"arm":arm,"adaptation":{"localBehavior":true,"policy":"randomized fixed-horizon experiment; no adaptive reward"}}),
    ))
}

/// The delayed readout; a mature one is cached, published as a relation and announced once.
pub async fn report(a: &State, t: &str, id: &str) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    let r=sqlx::query("SELECT data,state,withdrawn,final_report,(now()>=ends_at+((data->>'settlementDays')::integer*interval '1 day')) AS settled FROM intelligence_experiments WHERE tenant=$1 AND id=$2 FOR UPDATE").bind(t).bind(id).fetch_optional(&mut *tx).await?.ok_or(bad("Experiment not found"))?;
    let d: Design =
        serde_json::from_value(r.get("data")).map_err(|_| bad("Invalid experiment design"))?;
    let state: String = r.get("state");
    let withdrawn: i32 = r.get("withdrawn");
    let settled: Option<bool> = r.get("settled");
    let cached: Option<Value> = r.get("final_report");
    if let Some(mut cached) = cached {
        if withdrawn > 0 || state == "stopped" {
            cached["causalUpliftProven"] = json!(false);
            cached["withdrawals"] = json!(withdrawn);
            cached["stoppedEarly"] = json!(state == "stopped");
            cached["finalLook"] = json!(false);
            cached["interval95"] = Value::Null;
        }
        tx.commit().await?;
        return Ok(cached);
    }
    let rows = sqlx::query(include_str!("experiments_report.sql"))
        .bind(t)
        .bind(id)
        .bind(&d.currency)
        .fetch_all(&mut *tx)
        .await?;
    let invalid = rows.iter().any(|r| r.get::<bool, _>("invalid_currency"));
    let values = rows
        .iter()
        .map(|r| {
            let net: i64 = r.get("net_minor");
            let baseline: f64 = r.get("baseline");
            (
                r.get::<i32, _>("arm") as usize,
                net.clamp(0, d.outcome_cap_minor) as f64 / d.outcome_cap_minor as f64
                    - d.cuped_theta * baseline,
            )
        })
        .collect::<Vec<_>>();
    let mature = settled == Some(true)
        && state != "draft"
        && state != "stopped"
        && withdrawn == 0
        && !invalid;
    let mut result = interval(&values, mature, d.minimum_per_arm, d.cuped_theta);
    result["id"] = json!(id);
    result["currency"] = json!(d.currency);
    result["currencyScale"] = json!(scale(&d.currency));
    result["withdrawals"] = json!(withdrawn);
    result["settled"] = json!(settled == Some(true));
    result["stoppedEarly"] = json!(state == "stopped");
    result["invalidCurrency"] = json!(invalid);
    result["netCollectedMinor"] = json!(
        rows.iter()
            .map(|r| i128::from(r.get::<i64, _>("net_minor")))
            .sum::<i128>()
            .to_string()
    );
    result["asOf"] = json!(chrono::Utc::now().to_rfc3339());
    if mature {
        sqlx::query(
            "UPDATE intelligence_experiments SET final_report=$3 WHERE tenant=$1 AND id=$2",
        )
        .bind(t)
        .bind(id)
        .bind(&result)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO knowledge_relations(tenant,kind,source_kind,source_id,target_kind,target_id,state,confidence,data,actor) VALUES($1,'EXPERIMENT_RESULT','experiment',$2,'channel',$3,'evidenced',1,$4,'native-payment-readout') ON CONFLICT(tenant,kind,source_id,target_id) DO UPDATE SET data=excluded.data").bind(t).bind(id).bind(&d.channel).bind(&result).execute(&mut *tx).await?;
        veyndo_core::outbox::publish_on(&mut *tx, t, "intelligence.experiment.result", &json!({"id":id,"channelId":d.channel,"causalUpliftProven":result["causalUpliftProven"],"asOf":result["asOf"]})).await?;
    }
    tx.commit().await?;
    Ok(result)
}

/// The shop figures a suggested preregistration starts from.
pub struct Basis<'a> {
    pub pricing_currency: &'a str,
    pub enabled: &'a [String],
    pub main_locale: &'a str,
    pub channel: &'a str,
}

/// A prefilled preregistration with the figures it is based on. Nothing is saved and no model is called.
pub async fn suggest(a: &State, t: &str, b: Basis<'_>) -> Result<Value> {
    let currency = if b.enabled.contains(&b.pricing_currency.to_owned()) {
        b.pricing_currency.to_owned()
    } else {
        b.enabled
            .first()
            .cloned()
            .ok_or(bad("Enable a currency before planning an experiment"))?
    };
    let channel = b.channel;
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
    let design = Design {
        title: format!("{control} vs {treatment}"),
        locale: b.main_locale.to_owned(),
        channel: channel.to_owned(),
        control: control.into(),
        treatment: treatment.into(),
        currency: currency.clone(),
        duration_hours: if payments >= 400 { 168 } else { 336 },
        settlement_days: 14,
        minimum_per_arm: 100,
        outcome_cap_minor: p95.clamp(1_000, 1_000_000_000_000),
        cuped_theta: 0.0,
        automation: Automation {
            finish: true,
            apply: false,
        },
    };
    design.validate()?;
    Ok(
        json!({"design":design,"basis":{"scope":"shop","livePayments30Days":payments,"p95PaymentMinor":p95,"currentMode":mode,"currency":currency,"scale":scale(&currency)},"warnings":warnings,"saved":false}),
    )
}
/// Worker lane step, at most every 30 seconds per process. Every write repeats the manual transition's own state check.
pub async fn auto_once(a: &State) -> Result<bool> {
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
        veyndo_core::outbox::publish_on(&a.db, &row.get::<String,_>("tenant"), "intelligence.experiment.changed", &json!({"id":row.get::<String,_>("id"),"operation":"knowledge.experiment.finish","actor":ACTOR})).await?;
    }
    let settled = sqlx::query("SELECT tenant,id,data FROM intelligence_experiments WHERE state='completed' AND withdrawn=0 AND coalesce((data->'automation'->>'finish')::boolean,false) AND (final_report IS NULL OR NOT final_report ? 'decision') AND now()>=ends_at+((data->>'settlementDays')::integer*interval '1 day') ORDER BY ends_at LIMIT 50")
        .fetch_all(&a.db).await?;
    for row in settled {
        let (t, id): (String, String) = (row.get("tenant"), row.get("id"));
        let outcome = async {
            let design: Design = serde_json::from_value(row.get("data"))
                .map_err(|_| bad("Invalid stored experiment"))?;
            let result = report(a, &t, &id).await?;
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
            veyndo_core::outbox::publish_on(&mut *tx, &t, "intelligence.experiment.changed", &json!({"id":id,"operation":"knowledge.experiment.decide","decision":if publish {"published"} else {"kept"},"actor":ACTOR})).await?;
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
