//! Controlled experiments on the storefront layout: preregistration, transitions, assignment, the delayed readout from the payment ledger and the worker steps.
use crate::domain::experiment_model::{Automation, Design, interval};
use crate::types::events::{ExperimentChanged, ExperimentDecided, ExperimentResult};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::sync::atomic::{AtomicU64, Ordering};
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::{Error, Result, State, bad, conflict, hash};
use veyndo_pricing::types::currency::scale;

static LAST: AtomicU64 = AtomicU64::new(0);
const ACTOR: &str = "experiment-autopilot";

/// One registered experiment as the experiment list shows it; columns checked at compile time.
struct ListedExperiment {
    id: String,
    data: Value,
    revision: i64,
    state: String,
    started_at: Option<String>,
    ends_at: Option<String>,
}

/// The registered experiments of a shop, newest first.
pub async fn list(a: &State, t: &str) -> Result<Value> {
    let rows = sqlx::query_as!(
        ListedExperiment,
        "SELECT id,data,revision,state,started_at::text,ends_at::text FROM intelligence_experiments WHERE tenant=$1 ORDER BY created_at DESC LIMIT 100",
        t
    )
    .fetch_all(&a.db)
    .await?;
    Ok(
        json!({"experiments":rows.iter().map(|r|json!({"id":r.id,"design":r.data,"revision":r.revision,"state":r.state,"startedAt":r.started_at,"endsAt":r.ends_at})).collect::<Vec<_>>() }),
    )
}

/// The JSON fields `change` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ChangeInput {
    design: Value,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
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
    let input: ChangeInput = view(v);
    let mut tx = a.db.begin().await?;
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,24))", t)
        .execute(&mut *tx)
        .await?;
    let id = if name == "knowledge.experiment.create" {
        let design: Design = serde_json::from_value(input.design.clone())
            .map_err(|_| bad("Invalid experiment design"))?;
        design.validate()?;
        let count = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM intelligence_experiments WHERE tenant=$1"#,
            t
        )
        .fetch_one(&mut *tx)
        .await?;
        if count >= 100 {
            return Err(bad("Experiment history limit reached"));
        }
        let exists = veyndo_marketing::store::behavior_channels::channel_exists(
            &mut *tx,
            t,
            &design.channel,
        )
        .await?;
        if !exists {
            return Err(bad("Channel does not belong to shop"));
        }
        if !currencies.contains(&design.currency) {
            return Err(bad("Experiment currency is not enabled"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query!("INSERT INTO intelligence_experiments(tenant,id,channel,data,state,actor) VALUES($1,$2,$3,$4,'draft',$5)",
            t, id, design.channel, json!(design), actor).execute(&mut *tx).await?;
        id
    } else {
        let id = input.id.as_deref().ok_or(bad("Experiment required"))?;
        let r=sqlx::query!("SELECT data,revision,state FROM intelligence_experiments WHERE tenant=$1 AND id=$2 FOR UPDATE",
            t, id).fetch_optional(&mut *tx).await?.ok_or(bad("Experiment not found"))?;
        if input.revision != Some(r.revision) {
            return Err(conflict("Experiment revision changed"));
        }
        let state = r.state;
        if name == "knowledge.experiment.start" && state == "draft" {
            let d: Design =
                serde_json::from_value(r.data).map_err(|_| bad("Invalid stored experiment"))?;
            d.validate()?;
            let active=sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running') AS "active!""#,
                t, d.channel).fetch_one(&mut *tx).await?;
            if active {
                return Err(conflict("A channel experiment is already running"));
            }
            sqlx::query!("UPDATE intelligence_experiments SET state='running',started_at=now(),ends_at=now()+($3*interval '1 hour'),revision=revision+1 WHERE tenant=$1 AND id=$2",
                t, id, d.duration_hours as f64).execute(&mut *tx).await?;
        } else if name == "knowledge.experiment.finish" && state == "running" {
            let elapsed = sqlx::query_scalar!(
                r#"SELECT ends_at<=now() AS "elapsed!" FROM intelligence_experiments WHERE tenant=$1 AND id=$2"#,
                t,
                id
            )
            .fetch_one(&mut *tx)
            .await?;
            if !elapsed {
                return Err(conflict("Preregistered horizon has not elapsed"));
            }
            sqlx::query!("UPDATE intelligence_experiments SET state='completed',revision=revision+1 WHERE tenant=$1 AND id=$2",
                t, id).execute(&mut *tx).await?;
        } else if name == "knowledge.experiment.stop" && state == "running" {
            sqlx::query!("UPDATE intelligence_experiments SET state='stopped',revision=revision+1 WHERE tenant=$1 AND id=$2",
                t, id).execute(&mut *tx).await?;
            sqlx::query!("UPDATE knowledge_relations SET data=jsonb_set(jsonb_set(jsonb_set(data,'{causalUpliftProven}','false'),'{interval95}','null'),'{stoppedEarly}','true') WHERE tenant=$1 AND kind='EXPERIMENT_RESULT' AND source_id=$2",
                t, id).execute(&mut *tx).await?;
        } else {
            return Err(bad(
                "Invalid experiment transition; design is immutable after registration",
            ));
        }
        id.to_owned()
    };
    let changed = ExperimentChanged {
        id: &id,
        operation: name,
        actor,
    };
    veyndo_core::events::emit(&mut *tx, t, &changed).await?;
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
    let r=sqlx::query!("SELECT id,data FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running' AND ends_at>now() FOR UPDATE",
        s.tenant, s.channel).fetch_optional(&mut **tx).await?;
    let Some(r) = r else { return Ok(None) };
    let id = r.id;
    let d: Design = serde_json::from_value(r.data).map_err(|_| bad("Invalid stored experiment"))?;
    if !s.open || s.currency != d.currency {
        return Ok(None);
    };
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM intelligence_assignments WHERE tenant=$1 AND experiment_id=$2"#,
        s.tenant,
        id
    )
    .fetch_one(&mut **tx)
    .await?;
    let saved=sqlx::query_scalar!("SELECT arm FROM intelligence_assignments WHERE tenant=$1 AND experiment_id=$2 AND cart_id=$3",
        s.tenant, id, s.cart).fetch_optional(&mut **tx).await?;
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
        let baseline=sqlx::query_scalar!(r#"SELECT coalesce(sum(views),0)::bigint AS "views!" FROM session_signals WHERE tenant=$1 AND session=$2"#,
            s.tenant, hash(s.cart)).fetch_one(&mut **tx).await?;
        sqlx::query!("INSERT INTO intelligence_assignments(tenant,experiment_id,cart_id,arm,baseline) VALUES($1,$2,$3,$4,$5)",
            s.tenant, id, s.cart, arm, baseline.clamp(0,1000) as f64/1000.0).execute(&mut **tx).await?;
        arm
    };
    Ok(Some(
        json!({"variant":if arm==0{d.control}else{d.treatment},"propensity":0.5,"experimentId":id,"arm":arm,"adaptation":{"localBehavior":true,"policy":"randomized fixed-horizon experiment; no adaptive reward"}}),
    ))
}

/// The delayed readout; a mature one is cached, published as a relation and announced once.
pub async fn report(a: &State, t: &str, id: &str) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    let r=sqlx::query!("SELECT data,state,withdrawn,final_report,(now()>=ends_at+((data->>'settlementDays')::integer*interval '1 day')) AS settled FROM intelligence_experiments WHERE tenant=$1 AND id=$2 FOR UPDATE",
        t, id).fetch_optional(&mut *tx).await?.ok_or(bad("Experiment not found"))?;
    let d: Design = serde_json::from_value(r.data).map_err(|_| bad("Invalid experiment design"))?;
    let state = r.state;
    let withdrawn = r.withdrawn;
    let settled = r.settled;
    let cached = r.final_report;
    if let Some(mut cached) = cached {
        if withdrawn > 0 || state == "stopped" {
            merge(
                &mut cached,
                Invalidated {
                    causal_uplift_proven: false,
                    withdrawals: withdrawn,
                    stopped_early: state == "stopped",
                    final_look: false,
                    interval95: Value::Null,
                },
            )?;
        }
        tx.commit().await?;
        return Ok(cached);
    }
    let rows = sqlx::query_file!("src/store/experiments_report.sql", t, id, d.currency)
        .fetch_all(&mut *tx)
        .await?;
    let invalid = rows.iter().any(|r| r.invalid_currency);
    let values = rows
        .iter()
        .map(|r| {
            let net = r.net_minor;
            let baseline = r.baseline;
            (
                r.arm as usize,
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
    let as_of = chrono::Utc::now().to_rfc3339();
    merge(
        &mut result,
        ReadoutFacts {
            id,
            currency: &d.currency,
            currency_scale: scale(&d.currency),
            withdrawals: withdrawn,
            settled: settled == Some(true),
            stopped_early: state == "stopped",
            invalid_currency: invalid,
            net_collected_minor: rows
                .iter()
                .map(|r| i128::from(r.net_minor))
                .sum::<i128>()
                .to_string(),
            as_of: &as_of,
        },
    )?;
    if mature {
        sqlx::query!(
            "UPDATE intelligence_experiments SET final_report=$3 WHERE tenant=$1 AND id=$2",
            t,
            id,
            result
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("INSERT INTO knowledge_relations(tenant,kind,source_kind,source_id,target_kind,target_id,state,confidence,data,actor) VALUES($1,'EXPERIMENT_RESULT','experiment',$2,'channel',$3,'evidenced',1,$4,'native-payment-readout') ON CONFLICT(tenant,kind,source_id,target_id) DO UPDATE SET data=excluded.data",
            t, id, d.channel, result).execute(&mut *tx).await?;
        let readout = ExperimentResult {
            id,
            channel_id: &d.channel,
            causal_uplift_proven: view::<Readout>(&result).causal_uplift_proven.as_bool(),
            as_of: &as_of,
        };
        veyndo_core::events::emit(&mut *tx, t, &readout).await?;
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
    let row = sqlx::query!(r#"SELECT count(*) AS "payments!",coalesce(percentile_cont(0.95) WITHIN GROUP (ORDER BY p.amount_minor),0)::bigint AS "p95!" FROM payment_attempts p WHERE p.tenant=$1 AND p.currency=$2 AND p.environment='live' AND p.state IN ('captured','captured_late') AND p.capture_confirmed_at>now()-interval '30 days'"#,
        t, currency).fetch_one(&a.db).await?;
    let (payments, p95) = (row.payments, row.p95);
    let mode = sqlx::query_scalar!(
        r#"SELECT coalesce(data->>'mode','balanced') AS "mode!" FROM experiences WHERE tenant=$1"#,
        t
    )
    .fetch_optional(&a.db)
    .await?
    .unwrap_or_else(|| "balanced".into());
    let running = sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM intelligence_experiments WHERE tenant=$1 AND channel=$2 AND state='running') AS "running!""#,
        t, channel).fetch_one(&a.db).await?;
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
/// What a cached readout becomes once withdrawals or an early stop invalidate it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Invalidated {
    causal_uplift_proven: bool,
    withdrawals: i32,
    stopped_early: bool,
    final_look: bool,
    interval95: Value,
}

/// The facts a readout records beside its interval.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadoutFacts<'a> {
    id: &'a str,
    currency: &'a str,
    currency_scale: Option<u8>,
    withdrawals: i32,
    settled: bool,
    stopped_early: bool,
    invalid_currency: bool,
    net_collected_minor: String,
    as_of: &'a str,
}

/// The readout flags the worker decides on.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Readout {
    final_look: Value,
    causal_uplift_proven: Value,
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
    let due = sqlx::query!("UPDATE intelligence_experiments SET state='completed',revision=revision+1 WHERE state='running' AND ends_at<=now() AND coalesce((data->'automation'->>'finish')::boolean,false) RETURNING tenant,id")
        .fetch_all(&a.db).await?;
    for row in due {
        worked = true;
        let finished = ExperimentChanged {
            id: &row.id,
            operation: "knowledge.experiment.finish",
            actor: ACTOR,
        };
        veyndo_core::events::emit(&a.db, &row.tenant, &finished).await?;
    }
    let settled = sqlx::query!("SELECT tenant,id,data FROM intelligence_experiments WHERE state='completed' AND withdrawn=0 AND coalesce((data->'automation'->>'finish')::boolean,false) AND (final_report IS NULL OR NOT final_report ? 'decision') AND now()>=ends_at+((data->>'settlementDays')::integer*interval '1 day') ORDER BY ends_at LIMIT 50")
        .fetch_all(&a.db).await?;
    for row in settled {
        let (t, id) = (row.tenant, row.id);
        let outcome = async {
            let design: Design =
                serde_json::from_value(row.data).map_err(|_| bad("Invalid stored experiment"))?;
            let result = report(a, &t, &id).await?;
            let readout: Readout = view(&result);
            if readout.final_look != true {
                return Ok(false);
            }
            let publish = design.automation.apply && readout.causal_uplift_proven == true;
            let mut tx = a.db.begin().await?;
            let decided = sqlx::query!("UPDATE intelligence_experiments SET final_report=final_report||$3 WHERE tenant=$1 AND id=$2 AND final_report IS NOT NULL AND NOT final_report ? 'decision'",
                t, id, json!({"decision":if publish {"published"} else {"kept"},"decidedAt":chrono::Utc::now().to_rfc3339(),"decidedBy":ACTOR}))
                .execute(&mut *tx).await?.rows_affected();
            if decided != 1 {
                return Ok(false);
            }
            if publish {
                // The proved layout becomes the shop's fixed layout; without proof the shop stays as it is.
                sqlx::query!("UPDATE experiences SET data=jsonb_set(data,'{mode}',to_jsonb($2::text)),revision=revision+1 WHERE tenant=$1",
                    t, design.treatment).execute(&mut *tx).await?;
            }
            let decided = ExperimentDecided {
                id: &id,
                operation: "knowledge.experiment.decide",
                decision: if publish { "published" } else { "kept" },
                actor: ACTOR,
            };
            veyndo_core::events::emit(&mut *tx, &t, &decided).await?;
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
