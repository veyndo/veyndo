//! Durable agent runs in storage: the caller projection, the conversation turn fence, publication under the run lease and the checkpoint journal.
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode, conflict};

/// The lease a worker holds on one running task.
#[derive(Clone, Copy)]
pub struct Lease<'a> {
    pub tenant: &'a str,
    pub id: &'a str,
    pub lease: &'a str,
}

/// Share one conversation turn fence between explicit continuation and approved work.
pub async fn claim_conversation(
    tx: &mut Transaction<'_, Postgres>,
    t: &str,
    conversation: &str,
    task: &str,
) -> Result<()> {
    let row=sqlx::query("SELECT turn_owner,(turn_until IS NULL OR turn_until<now()) AS available FROM conversations WHERE tenant=$1 AND id=$2 FOR UPDATE").bind(t).bind(conversation).fetch_optional(&mut **tx).await?.ok_or(conflict("Conversation unavailable"))?;
    let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks WHERE tenant=$1 AND conversation_id=$2 AND id<>$3 AND run_state IN ('queued','running','uncertain'))").bind(t).bind(conversation).bind(task).fetch_one(&mut **tx).await?;
    if busy
        || (row.get::<Option<String>, _>("turn_owner").as_deref() != Some(task)
            && !row.get::<bool, _>("available"))
    {
        return Err(conflict("A newer conversation turn is active"));
    }
    sqlx::query(
        "UPDATE conversations SET turn_owner=$3,turn_until='infinity' WHERE tenant=$1 AND id=$2",
    )
    .bind(t)
    .bind(conversation)
    .bind(task)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
/// What a caller may see of a run; checkpoints and credentials stay inside.
pub fn project(row: &PgRow) -> Value {
    let state: String = row.get("run_state");
    let cp: Value = row.get("run_checkpoint");
    let request: Value = row.get("run_request");
    let execution=request["execution"].as_array().map(|plan|json!({"total":plan.len(),"completed":cp.as_object().map_or(0,|o|o.keys().filter(|k|k.starts_with("operation-")).count()),"steps":plan.iter().enumerate().map(|(n,s)|json!({"name":s["name"],"label":s["label"],"completed":cp.get(format!("operation-{n}")).is_some(),"translationJobId":cp.get(format!("translation-{n}"))})).collect::<Vec<_>>()}));
    let steps = cp
        .as_object()
        .map_or(0, |o| o.keys().filter(|k| k.starts_with("model-")).count());
    let scenario = request.get("scenario").map(|s| {
        let mut result = s.clone();
        result["steps"] = cp["scenario-state"]["steps"].clone();
        result
    });
    json!({"taskId":row.get::<String,_>("id"),"agentId":row.get::<Value,_>("run_request")["agentId"],"channel":request["channel"],"state":state,"revision":row.get::<i64,_>("run_revision"),"completedModelSteps":steps,"scenario":scenario,"execution":execution,"error":row.get::<Option<Value>,_>("run_error"),"conversationId":row.get::<Option<String>,_>("conversation_id"),"approvalRequired":!row.get::<bool,_>("applied"),"preview":if state=="ready" {row.get::<Value,_>("proposal")}else{Value::Null},"updatedAt":row.get::<String,_>("updated_text")})
}

/// Publishes the finished proposal while the lease is still held, and releases the conversation turn.
/// The published evidence of an agent run.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunEvidence {
    #[serde(deserialize_with = "lenient")]
    proposal: ProposalSummary,
}

/// The summary of a proposal.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProposalSummary {
    #[serde(deserialize_with = "lenient")]
    summary: Option<String>,
}

pub async fn publish(a: &State, t: &str, id: &str, evidence: &Value, lease: &str) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let row=sqlx::query("UPDATE tasks SET proposal=$1,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$2 AND id=$3 AND run_state='running' AND run_lease=$4 AND run_until>now() RETURNING conversation_id")
        .bind(evidence).bind(t).bind(id).bind(lease).fetch_optional(&mut *tx).await?.ok_or(conflict("Agent run cancelled or lease lost"))?;
    if let Some(c) = row.get::<Option<String>, _>("conversation_id") {
        sqlx::query("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3").bind(t).bind(&c).bind(id).execute(&mut *tx).await?;
        let data = json!({"taskId":id,"preview":evidence,"approvalRequired":true});
        sqlx::query("INSERT INTO chat_messages(tenant,conversation_id,role,content,data) VALUES($1,$2,'assistant',$3,$4)").bind(t).bind(c).bind(view::<RunEvidence>(evidence).proposal.summary.as_deref().unwrap_or("")).bind(data).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// One saved checkpoint of a running task.
pub async fn load(a: &State, r: Lease<'_>, key: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar("SELECT run_checkpoint->$4 FROM tasks WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()")
        .bind(r.tenant).bind(r.id).bind(r.lease).bind(key).fetch_optional(&a.db).await?.flatten())
}

/// Saves a checkpoint and extends the lease; a lost lease is a conflict.
pub async fn save(a: &State, r: Lease<'_>, key: &str, value: &Value) -> Result<()> {
    let changed=sqlx::query("UPDATE tasks SET run_checkpoint=jsonb_set(run_checkpoint,ARRAY[$4],$5,true),run_pending=NULL,run_revision=run_revision+1,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()")
        .bind(r.tenant).bind(r.id).bind(r.lease).bind(key).bind(value).execute(&a.db).await?.rows_affected();
    if changed != 1 {
        return Err(conflict("Agent run cancelled or lease lost"));
    }
    Ok(())
}

/// Marks an inference call as pending before the provider is called, so an ambiguous outcome stays visible.
pub async fn pending(a: &State, r: Lease<'_>, key: &str) -> Result<()> {
    let changed=sqlx::query("UPDATE tasks SET run_pending=$4,run_updated=now(),run_until=now()+interval '240 seconds' WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now()")
        .bind(r.tenant).bind(r.id).bind(r.lease).bind(key).execute(&a.db).await?.rows_affected();
    if changed != 1 {
        return Err(conflict("Agent lease lost before inference"));
    }
    Ok(())
}

/// Requeues expired runs; a run with a pending provider call becomes uncertain instead, since its outcome is unknown.
pub async fn recover(a: &State) -> Result<()> {
    sqlx::query("UPDATE tasks SET run_state=CASE WHEN run_pending IS NULL THEN 'queued' ELSE 'uncertain' END,run_error=CASE WHEN run_pending IS NULL THEN NULL ELSE jsonb_build_object('code',CASE WHEN run_request ? 'execution' THEN 'agent_work_interrupted' ELSE 'agent_model_interrupted' END,'retryable',false) END,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE (tenant,id) IN (SELECT tenant,id FROM tasks WHERE run_state='running' AND run_until<now() ORDER BY run_updated LIMIT 20 FOR UPDATE SKIP LOCKED)").execute(&a.db).await?;
    Ok(())
}
/// Claims the next queued run under a new lease: one ordinary run or two scenario runs per shop at a time.
pub async fn claim(a: &State, lease: &str) -> Result<Option<PgRow>> {
    let mut tx = a.db.begin().await?;
    let row=sqlx::query("SELECT * FROM tasks r WHERE run_state='queued' AND run_after<=now() AND NOT EXISTS(SELECT 1 FROM tasks active WHERE active.tenant=r.tenant AND active.run_state='running' AND active.run_until>now() AND NOT(active.run_request ? 'scenario')) AND (SELECT count(*) FROM tasks active WHERE active.tenant=r.tenant AND active.run_state='running' AND active.run_until>now()) < CASE WHEN r.run_request ? 'scenario' THEN 2 ELSE 1 END ORDER BY run_updated,tenant,id FOR UPDATE SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let t: String = row.get("tenant");
    let id: String = row.get("id");
    // Serialize tenant admission across replicas, then recheck the running count.
    let admitted: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,2))")
            .bind(&t)
            .fetch_one(&mut *tx)
            .await?;
    if !admitted {
        return Ok(None);
    };
    let is_scenario = row.get::<Value, _>("run_request")["scenario"].is_object();
    let busy:bool=sqlx::query_scalar("SELECT count(*) >= $2 OR coalesce(bool_or(NOT(run_request ? 'scenario')),false) FROM tasks WHERE tenant=$1 AND run_state='running' AND run_until>now()")
        .bind(&t).bind(if is_scenario {2_i64}else{1_i64}).fetch_one(&mut *tx).await?;
    if busy {
        return Ok(None);
    };
    sqlx::query("UPDATE tasks SET run_state='running',run_checkpoint=run_checkpoint-'app-views',run_lease=$3,run_until=now()+interval '240 seconds',run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2")
        .bind(&t).bind(&id).bind(lease).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(row))
}
/// The actor-private problem a failed run records; outbox projections carry IDs and states only.
pub fn problem(error: &Error) -> Value {
    let code = match error.0 {
        StatusCode::FORBIDDEN => "agent_authority_changed",
        StatusCode::TOO_MANY_REQUESTS => "agent_capacity_busy",
        _ => "agent_step_failed",
    };
    json!({"code":code,"status":error.0.as_u16(),"detail":error.1.chars().take(500).collect::<String>()})
}
/// Stops a failed run under its lease, tells its conversation and releases a finished turn.
pub async fn fail(a: &State, r: Lease<'_>, error: &Error) -> Result<()> {
    let problem = problem(error);
    let mut tx = a.db.begin().await?;
    let stopped=sqlx::query("UPDATE tasks SET run_state=CASE WHEN run_pending IS NOT NULL THEN 'uncertain' ELSE 'failed' END,run_error=$4,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$1 AND id=$2 AND run_lease=$3 AND run_state='running' AND run_until>now() RETURNING conversation_id,run_state")
        .bind(r.tenant).bind(r.id).bind(r.lease).bind(&problem).fetch_optional(&mut *tx).await?;
    if let Some(stopped) = stopped
        && let Some(c) = stopped.get::<Option<String>, _>("conversation_id")
    {
        let state: String = stopped.get("run_state");
        let data = json!({"error":true,"runId":r.id,"runState":state,"code":problem["code"],"providerUnavailable":error.0==StatusCode::BAD_GATEWAY});
        sqlx::query("INSERT INTO chat_messages(tenant,conversation_id,role,content,data) VALUES($1,$2,'assistant',$3,$4)").bind(r.tenant).bind(&c).bind(error.1.chars().take(500).collect::<String>()).bind(data).execute(&mut *tx).await?;
        if state == "failed" {
            sqlx::query("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3").bind(r.tenant).bind(&c).bind(r.id).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
