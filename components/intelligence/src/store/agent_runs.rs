//! Durable agent runs in storage: the caller projection, the conversation turn fence, publication under the run lease and the checkpoint journal.
use serde_json::{Value, json};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use veyndo_core::{Result, State, conflict};

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
pub async fn publish(a: &State, t: &str, id: &str, evidence: &Value, lease: &str) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let row=sqlx::query("UPDATE tasks SET proposal=$1,run_state='ready',run_pending=NULL,run_lease=NULL,run_until=NULL,run_revision=run_revision+1,run_updated=now() WHERE tenant=$2 AND id=$3 AND run_state='running' AND run_lease=$4 AND run_until>now() RETURNING conversation_id")
        .bind(evidence).bind(t).bind(id).bind(lease).fetch_optional(&mut *tx).await?.ok_or(conflict("Agent run cancelled or lease lost"))?;
    if let Some(c) = row.get::<Option<String>, _>("conversation_id") {
        sqlx::query("UPDATE conversations SET turn_owner=NULL,turn_until=NULL WHERE tenant=$1 AND id=$2 AND turn_owner=$3").bind(t).bind(&c).bind(id).execute(&mut *tx).await?;
        let data = json!({"taskId":id,"preview":evidence,"approvalRequired":true});
        sqlx::query("INSERT INTO chat_messages(tenant,conversation_id,role,content,data) VALUES($1,$2,'assistant',$3,$4)").bind(t).bind(c).bind(evidence["proposal"]["summary"].as_str().unwrap_or("")).bind(data).execute(&mut *tx).await?;
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
