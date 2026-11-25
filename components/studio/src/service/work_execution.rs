//! Approved work runs under the existing task lease; each native write is fenced against cancellation and recorded before advancing. Translation steps reuse native draft storage and yield the worker while drafts are prepared.
use crate::domain::proposal::Step;
use crate::ports::WorkHost;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_catalog::domain::selection::parse as parse_selection;
use veyndo_catalog::http::translation_jobs;
use veyndo_catalog::service::translation_apply::admit_selection;
use veyndo_cognition::inference::{Choice, Inference};
use veyndo_core::document::{lenient, view};
use veyndo_core::store::history_context::context;
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::permit;
use veyndo_identity::store::agent_authority::headers as authority;
use veyndo_intelligence::service::agent_runs::{CURRENT, Run};
use veyndo_intelligence::service::run_journal::{authorize, load};
use veyndo_shop::Settings;
use veyndo_shop::service::read_cache::config;

/// Both explicit settings commands and approved merchant tasks enqueue the same durable draft owner.
pub async fn enqueue_translation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    s: &Settings,
    request: (&str, &Choice, bool, Option<&[String]>),
) -> Result<()> {
    let (target, choice, overwrite, products) = request;
    translation_jobs::enqueue(
        tx,
        t,
        id,
        (&s.main_locale, &s.locales),
        (target, &json!(choice), overwrite, products),
    )
    .await
}
/// A translation provider must be centrally configured before a backend is taken.
pub async fn admit_provider(inference: &Inference, choice: &Choice) -> Result<()> {
    let available = inference.public_providers().await.map_err(bad)?;
    if !available["providers"].as_array().is_some_and(|providers| {
        providers
            .iter()
            .any(|p| p["id"] == json!(choice.provider) && p["configured"] == true)
    }) {
        return Err(bad("Translation provider is not configured"));
    }
    Ok(())
}
/// The JSON fields `execute` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ExecuteInput {
    execution: Value,
}

/// Executes approved work step by step under the task lease; each native write is fenced and recorded before advancing.
pub async fn execute<H: WorkHost>(
    host: &H,
    t: &str,
    id: &str,
    h: &RequestContext,
    request: &Value,
) -> Result<Value> {
    let input: ExecuteInput = view(request);
    let a = host.models().core;
    let steps: Vec<Step> = serde_json::from_value(input.execution.clone())
        .map_err(|_| bad("Invalid approved work"))?;
    let run = CURRENT
        .try_with(Clone::clone)
        .map_err(|_| bad("Work requires its native worker"))?;
    for (index, step) in steps.iter().enumerate() {
        authorize(host).await?;
        let key = format!("operation-{index}");
        if load(host, &key).await?.is_some() {
            continue;
        }
        if step.name == "merchant.translations.create" {
            if !advance(host, t, id, (h, &run), index, step).await? {
                return Ok(json!({"taskId":id,"applied":false,"queued":true}));
            }
            continue;
        }
        let changed=sqlx::query("UPDATE tasks SET run_pending=$4,run_revision=run_revision+1 WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now()").bind(t).bind(id).bind(&run.lease).bind(&key).execute(&a.db).await?.rows_affected();
        if changed != 1 {
            return Err(conflict("Work cancelled before operation"));
        }
        let mut tx = a.db.begin().await?;
        // This transaction admits one operation. Managed saves and their receipts
        // commit together; native owners keep their own transaction below.
        let locked=sqlx::query("SELECT id FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() FOR UPDATE").bind(t).bind(id).bind(&run.lease).fetch_optional(&mut *tx).await?;
        if locked.is_none() {
            return Err(conflict("Work lost its operation lease"));
        }
        let args: Value = serde_json::from_str(&step.arguments_json)
            .map_err(|_| bad("Invalid approved arguments"))?;
        let admission = format!("admission-{key}");
        if let Some(change) = &step.binding {
            context(&mut tx, h, "agent-work").await?;
            host.apply_change(&mut tx, t, change, h).await?;
        } else {
            // Do not hold a backend while a native owner acquires its own one.
            // Retain an admission token across cancellation so an admitted native
            // effect can truthfully finish its receipt without reviving the task.
            let admitted = sqlx::query("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$5 AND run_until>now()")
                .bind(t).bind(id).bind(&admission).bind(json!({"lease":run.lease})).bind(&run.lease).execute(&mut *tx).await?.rows_affected();
            if admitted != 1 {
                return Err(conflict("Native work admission lost its lease"));
            }
            tx.commit().await?;
            Box::pin(host.read(h, &step.name, &args)).await?;
            tx = a.db.begin().await?;
        }
        // Keep only outcome metadata; domain data stays in its existing authorized owner.
        let receipt = json!({"index":index,"name":step.name,"label":step.label,"completed":true});
        let recorded = sqlx::query("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint-$6::text,ARRAY[$4],$5,true),run_pending=CASE WHEN run_state='running' THEN NULL ELSE run_pending END,run_revision=run_revision+1,run_updated=now(),run_until=CASE WHEN run_state='running' AND run_until>now() THEN now()+interval '240 seconds' ELSE run_until END WHERE tenant=$1 AND id=$2 AND ((run_lease=$3 AND run_state='running' AND run_until>now()) OR ($7 AND run_checkpoint->$6=jsonb_build_object('lease',$3::text) AND ((run_state IN ('cancelled','uncertain') AND run_lease IS NULL) OR (run_state='running' AND run_lease=$3))))")
            .bind(t).bind(id).bind(&run.lease).bind(&key).bind(receipt).bind(&admission).bind(step.binding.is_none()).execute(&mut *tx).await?.rows_affected();
        if recorded != 1 {
            return Err(conflict("Work lost its completion lease"));
        }
        tx.commit().await?;
    }
    let mut tx = a.db.begin().await?;
    let row=sqlx::query("UPDATE tasks SET applied=true,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() RETURNING conversation_id").bind(t).bind(id).bind(&run.lease).fetch_optional(&mut *tx).await?.ok_or(conflict("Work completion lost its lease"))?;
    if let Some(c) = row.get::<Option<String>, _>("conversation_id") {
        sqlx::query("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3").bind(t).bind(c).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(json!({"taskId":id,"applied":true}))
}
/// The JSON fields `request` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RequestInput {
    #[serde(deserialize_with = "lenient")]
    target_locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    overwrite: Option<bool>,
}

/// The target, provider, overwrite flag and product selection of a translation step.
fn request(v: &Value) -> Result<(String, Choice, bool, Option<Vec<String>>)> {
    let input: RequestInput = view(v);
    Ok((
        input
            .target_locale
            .as_deref()
            .ok_or(bad("Target language required"))?
            .into(),
        serde_json::from_value(
            v.get("inference")
                .cloned()
                .unwrap_or(json!({"provider":"platform","model":null})),
        )
        .map_err(|_| bad("Invalid translation provider"))?,
        input.overwrite.unwrap_or(false),
        parse_selection(v.get("productIds"))?,
    ))
}

/// Checks a translation step: a configured provider, an explicit selection and an enabled target.
pub async fn validate_translation<H: WorkHost>(
    host: &H,
    t: &str,
    v: &Value,
    created: &[String],
) -> Result<()> {
    let a = host.models().core;
    let (target, choice, _, products) = request(v)?;
    admit_provider(host.models().inference, &choice).await?;
    // Collection work must never silently expand to a whole catalogue.
    if products.is_none() {
        return Err(bad(
            "Collection work requires an explicit product selection",
        ));
    }
    let (s, _) = config(a, t).await?;
    if target == s.main_locale || !s.locales.contains(&target) {
        return Err(bad("Select an enabled translation target"));
    }
    let existing = products
        .unwrap()
        .into_iter()
        .filter(|id| !created.contains(id))
        .collect::<Vec<_>>();
    if existing.is_empty() {
        return Ok(());
    }
    let mut tx = a.db.begin().await?;
    admit_selection(&mut tx, t, Some(&existing)).await
}

/// The saved authority of a work task with a running translation step.
pub async fn task_headers(a: &State, t: &str, task: &str) -> Result<RequestContext> {
    Ok(task_context(a, t, task).await?.0)
}
/// The saved authority and trace of a work task with a running translation step.
pub async fn task_context(a: &State, t: &str, task: &str) -> Result<(RequestContext, String)> {
    let row = sqlx::query("SELECT * FROM tasks WHERE tenant=$1 AND id=$2 AND run_state IN ('queued','running') AND run_request ? 'execution'")
        .bind(t).bind(task).fetch_optional(&a.db).await?.ok_or(conflict("Parent work is stopped"))?;
    let h = authority(a, &row).await?;
    permit(&h, "catalog.write")?;
    Ok((h, row.get("run_trace")))
}

/// Advances a translation step: queues its drafts once, then yields the worker until they are ready.
async fn advance<H: WorkHost>(
    host: &H,
    t: &str,
    task: &str,
    (h, run): (&RequestContext, &Run),
    index: usize,
    step: &Step,
) -> Result<bool> {
    let a = host.models().core;
    let key = format!("operation-{index}");
    let job_id = format!("work-{task}-{index}");
    let v: Value =
        serde_json::from_str(&step.arguments_json).map_err(|_| bad("Invalid translation work"))?;
    let (target, choice, overwrite, products) = request(&v)?;
    let (settings, _) = config(a, t).await?;
    let mut tx = a.db.begin().await?;
    let locked = sqlx::query("SELECT id FROM tasks WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_lease=$3 AND run_until>now() FOR UPDATE")
        .bind(t).bind(task).bind(&run.lease).fetch_optional(&mut *tx).await?;
    if locked.is_none() {
        return Err(conflict("Translation work lost its task lease"));
    }
    let job = sqlx::query("SELECT status,error,error_code FROM translation_jobs WHERE tenant=$1 AND id=$2 AND run_task=$3 FOR UPDATE")
        .bind(t).bind(&job_id).bind(task).fetch_optional(&mut *tx).await?;
    let status = if let Some(job) = job {
        let state: String = job.get("status");
        if state == "failed"
            && job.get::<Option<String>, _>("error_code").as_deref() == Some("call_interrupted")
        {
            sqlx::query("UPDATE tasks SET run_pending=$3 WHERE tenant=$1 AND id=$2")
                .bind(t)
                .bind(task)
                .bind(format!("translation-{index}"))
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Err(conflict(
                "Translation call may already have been processed or charged; review before explicitly continuing",
            ));
        }
        if ["failed", "cancelled"].contains(&state.as_str()) {
            return Err(conflict(
                "Translation drafts stopped; inspect the linked job and explicitly resume this task",
            ));
        }
        state
    } else {
        permit(h, "catalog.write")?;
        enqueue_translation(
            &mut tx,
            t,
            &job_id,
            &settings,
            (&target, &choice, overwrite, products.as_deref()),
        )
        .await?;
        sqlx::query("UPDATE translation_jobs SET run_task=$3 WHERE tenant=$1 AND id=$2")
            .bind(t)
            .bind(&job_id)
            .bind(task)
            .execute(&mut *tx)
            .await?;
        "queued".into()
    };
    sqlx::query("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true) WHERE tenant=$1 AND id=$2")
        .bind(t).bind(task).bind(format!("translation-{index}")).bind(json!(job_id)).execute(&mut *tx).await?;
    if status == "ready" {
        let receipt = json!({"index":index,"name":step.name,"label":step.label,"completed":true,"translationJobId":job_id});
        sqlx::query("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$3],$4,true),run_revision=run_revision+1,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$5")
            .bind(t).bind(task).bind(&key).bind(receipt).bind(&run.lease).execute(&mut *tx).await?;
    } else {
        // Yield the existing worker instead of holding a connection or busy-waiting.
        sqlx::query("UPDATE tasks SET run_state='queued',run_after=now()+interval '5 seconds',run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_state='running' AND run_until>now() AND run_lease=$3")
            .bind(t).bind(task).bind(&run.lease).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(status == "ready")
}
