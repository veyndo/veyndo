//! Native job completion and authenticated private record evidence share one transaction and current creator/package fences.
use crate::domain::{action_admission, approval, callback_grant, core_extensions, input_schema};
use crate::ports::RecordRules;
use crate::store::package_fence::lock_package_write;
use crate::store::{managed_data, records};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::Entity;
use veyndo_app_model::admission::callback_scope;
use veyndo_app_model::manifest::Manifest;
use veyndo_cognition::inference::settings::{open, seal};
use veyndo_core::document::{lenient, view};
use veyndo_core::store::history_context::context;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_identity::{merchant, permit};

fn binding(t: &str, m: &Manifest, entity: &str, id: &str) -> String {
    format!(
        "job-evidence-v1:{t}:{}:{entity}:{id}:{}",
        m.id,
        approval::canonical_digest(m)
    )
}
fn document(e: &Entity, v: &Value) -> Value {
    e.fields
        .iter()
        .filter(|f| f.name != "evidence")
        .map(|f| (f.name.clone(), v[&f.name].clone()))
        .collect()
}
/// A reported job completion: the calling service, the creator's current rights, the package, the job row and the report.
pub struct Completion<'a> {
    pub service: &'a RequestContext,
    pub creator: &'a RequestContext,
    pub manifest: &'a Manifest,
    pub job: &'a sqlx::postgres::PgRow,
    pub report: &'a Value,
}
/// Seals the completion evidence of a succeeded job into its reviewed private record.
pub async fn save(
    rules: &dyn RecordRules,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    completion: Completion<'_>,
) -> Result<()> {
    let Completion {
        service: h,
        creator: identity,
        manifest: m,
        job,
        report: v,
    } = completion;
    let Some(evidence) = v.get("evidence") else {
        return Ok(());
    };
    let report: CompletedReport = view(v);
    if h.principal.app.as_deref() != Some(m.id.as_str())
        || report.status.as_deref() != Some("succeeded")
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Only the approved job service can seal completion evidence".into(),
        ));
    }
    for p in ["jobs.write", "data.write"] {
        if !m.permissions.iter().any(|s| s == p) {
            return Err(bad("Job evidence capability not declared"));
        }
        callback_grant::permit(h, p)?;
        permit(identity, callback_scope(p).unwrap())?;
    }
    input_schema::validate_native(
        &json!({"type":"object","properties":{"action":{"type":"string","maxLength":32},"input":{"type":"object"}},"required":["action","input"],"additionalProperties":false}),
        evidence,
    )?;
    let save: EvidenceSave = view(evidence);
    let action = m
        .actions
        .iter()
        .find(|a| Some(&a.name) == save.action.as_ref() && a.handler == "save" && !a.public)
        .ok_or(bad("Private evidence save action required"))?;
    permit(
        identity,
        action.permission.as_deref().unwrap_or("apps.manage"),
    )?;
    action_admission::additional(identity, action)?;
    let entity = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == action.entity.as_ref() && !e.public_read)
        .ok_or(bad("Private evidence entity required"))?;
    core_extensions::access(entity, identity, true)?;
    let frozen: Value = job.get("input");
    let reviewed: ReviewedJob = view(&frozen);
    let mut input = rules.normalize(m, &save.input)?;
    let id = records::RecordMeta::of(&input)
        .id
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or(bad("Evidence record identity required"))?
        .to_owned();
    if reviewed.evidence_record != json!({"entity":entity.name,"id":id}) {
        return Err(conflict(
            "Evidence destination differs from the reviewed job",
        ));
    }
    let submitted: EvidenceRecord = view(&input);
    if view::<EvidenceFields>(&submitted.fields).result != report.result {
        return Err(conflict("Evidence result differs from the completed job"));
    }
    // Record fields are app-declared; only the host-owned `evidence` member is replaced.
    let fields = input
        .get_mut("fields")
        .and_then(Value::as_object_mut)
        .ok_or(bad("Evidence fields required"))?;
    fields.remove("evidence");
    let proof = json!({"v":1,"job":report.id,"actor":job.get::<String,_>("actor"),"action":job.get::<String,_>("action"),"inputHash":hash(&frozen.to_string()),"fieldsHash":hash(&document(entity,&Value::Object(fields.clone())).to_string()),"at":chrono::Utc::now().to_rfc3339()});
    fields.insert(
        "evidence".into(),
        json!(
            seal(&binding(t, m, &entity.name, &id), &proof.to_string()).map_err(|_| Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Job evidence encryption unavailable".into()
            ))?
        ),
    );
    input_schema::validate_native(&action.input_schema, &input)?;
    context(tx, identity, "app.job.completed").await?;
    managed_data::save_tx(rules, tx, t, m, entity, &input).await?;
    Ok(())
}
/// The evidence save a job service submits: the private save action and its record input.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EvidenceSave {
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    input: Value,
}

/// The submitted evidence record, as the completion check reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EvidenceRecord {
    fields: Value,
}

/// The record field the completed job result must match.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EvidenceFields {
    result: Value,
}

/// The reviewed job input fields evidence is checked against.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReviewedJob {
    evidence_record: Value,
}

/// The sealed completion proof, typed as it is checked.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Proof {
    v: Value,
    fields_hash: Value,
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    job: Value,
}

/// The completion report fields that evidence is bound to.
#[derive(Deserialize, Default)]
#[serde(default)]
struct CompletedReport {
    #[serde(deserialize_with = "lenient")]
    status: Option<String>,
    result: Value,
    id: Value,
}

/// The records a service asks to verify.
#[derive(Deserialize, Default)]
#[serde(default)]
struct EvidenceRequest {
    #[serde(deserialize_with = "lenient")]
    entity: Option<String>,
    #[serde(deserialize_with = "lenient")]
    ids: Vec<String>,
}

/// Verify actual current records, never caller-provided fields or booleans. Ordinary edits invalidate their evidence.
pub async fn verify(a: &State, h: &RequestContext, m: &Manifest, v: &Value) -> Result<Value> {
    input_schema::validate(
        &json!({"type":"object","properties":{"entity":{"type":"string","maxLength":32},"ids":{"type":"array","maxItems":100,"items":{"type":"string","maxLength":100}}},"required":["entity","ids"],"additionalProperties":false}),
        v,
    )?;
    callback_grant::permit(h, "data.read")?;
    if !m.permissions.iter().any(|s| s == "data.read") {
        return Err(bad("Evidence read capability not declared"));
    }
    let t = merchant(h)?;
    let request: EvidenceRequest = view(v);
    let e = m
        .entities
        .iter()
        .find(|e| Some(&e.name) == request.entity.as_ref() && !e.public_read)
        .ok_or(bad("Private evidence entity required"))?;
    core_extensions::access(e, h, false)?;
    let mut tx = a.db.begin().await?;
    lock_package_write(&mut tx, &t, m).await?;
    let mut verified = vec![];
    for id in &request.ids {
        let Some(row) = records::get(&mut tx, &t, &m.id, &e.name, id, false).await? else {
            continue;
        };
        let meta = records::RecordMeta::of(&row);
        let Some(token) = meta.evidence.as_deref().filter(|s| s.len() <= 2000) else {
            continue;
        };
        let Ok(raw) = open(&binding(&t, m, &e.name, id), token) else {
            continue;
        };
        let Ok(proof) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let proof: Proof = view(&proof);
        if proof.v != 1 || proof.fields_hash != hash(&document(e, &row).to_string()) {
            continue;
        }
        let Some(action) = m
            .actions
            .iter()
            .find(|a| Some(&a.name) == proof.action.as_ref() && a.handler == "job")
        else {
            continue;
        };
        permit(h, action.permission.as_deref().unwrap_or("apps.manage"))?;
        action_admission::additional(h, action)?;
        verified.push(json!({"id":id,"revision":meta.revision,"job":proof.job}));
    }
    tx.commit().await?;
    Ok(json!({"verified":verified}))
}
