//! Long-job service callbacks disclose input only after current actor consent, package and lease checks. Late callbacks cannot commit.
use crate::domain::{action_admission, approval, input_schema};
use crate::ports::Platform;
use crate::service::job_evidence;
use crate::store::package_fence::lock_package_write;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::localized::valid_locale_key;
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, uid};
use veyndo_identity::{merchant, permit};
use veyndo_kernel::verified_kernel;

/// The current rights of the member who created a job; a creator without access cannot have work completed.
async fn creator(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    actor: String,
) -> Result<RequestContext> {
    // Lock the scoped base rows, then read the canonical current projection.
    // FOR SHARE on the read-only view would require a needless UPDATE grant on it.
    sqlx::query("SELECT m.user_id FROM memberships m JOIN workspace_roles r ON r.tenant=m.tenant AND r.id=m.role WHERE m.tenant=$1 AND m.user_id=$2 FOR SHARE OF m,r")
        .bind(t).bind(&actor).fetch_optional(&mut **tx).await?
        .ok_or(Error(StatusCode::FORBIDDEN,"Job creator no longer has access".into()))?;
    let membership = sqlx::query(
        "SELECT role,permissions FROM membership_grants WHERE tenant=$1 AND user_id=$2 AND active",
    )
    .bind(t)
    .bind(&actor)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(Error(
        StatusCode::FORBIDDEN,
        "Job creator no longer has access".into(),
    ))?;
    let mut identity = RequestContext::new();
    identity.principal.user = Some(actor);
    identity.principal.role = Some(membership.get("role"));
    let rights: Value = membership.get("permissions");
    identity.principal.permissions = (!rights.is_null()).then(|| rights.to_string());
    Ok(identity)
}
/// The localized progress message of a report, bounded in languages and length.
fn message(v: &Value) -> Result<Value> {
    let message = if v["message"].is_null() {
        json!({})
    } else {
        v["message"].clone()
    };
    if !message.as_object().is_some_and(|o| {
        o.len() <= 100
            && o.iter()
                .all(|(k, v)| valid_locale_key(k) && v.as_str().is_some_and(|s| s.len() <= 500))
    }) {
        return Err(bad("Job messages require bounded localized text"));
    }
    Ok(message)
}
/// Claims a queued job or records its progress for the service that holds the lease.
pub async fn execute(
    a: &Platform,
    h: &RequestContext,
    m: &Manifest,
    op: &str,
    v: &Value,
) -> Result<Value> {
    let mut metadata = v.clone();
    metadata
        .as_object_mut()
        .ok_or(bad("App job callback must be an object"))?
        .remove("result");
    metadata.as_object_mut().unwrap().remove("evidence");
    input_schema::validate(
        &json!({"type":"object","properties":{"id":{"type":"string","maxLength":100},"lease":{"type":"string","maxLength":100},"progress":{"type":"integer","minimum":0,"maximum":100},"status":{"type":"string","enum":["running","succeeded","failed","cancelled"]},"message":{"type":"object"},"result":{"type":"object"},"checkpoint":{"type":"object"}},"required":["id"],"additionalProperties":false}),
        &metadata,
    )?;
    if v["result"].to_string().len() > 32768 || v["checkpoint"].to_string().len() > 32768 {
        return Err(bad("App job result exceeds 32 KiB"));
    }
    // Results are bounded stored documents, not executable action arguments.
    if let Some(result) = v.get("result") {
        input_schema::validate_native(&json!({"type":"object"}), result)?;
    }
    let t = merchant(h)?;
    let id = v["id"].as_str().unwrap();
    let mut tx = a.db.begin().await?;
    let scope = veyndo_app_storage::Scope {
        tenant: &t,
        app: &m.id,
    };
    let row = veyndo_app_storage::jobs::load(&mut tx, scope, id)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "App job not found".into()))?;
    if row.get::<String, _>("package_digest") != approval::canonical_digest(m) {
        return Err(conflict("Job package changed; review before retrying"));
    }
    let actor: String = row.get("actor");
    let identity = creator(&mut tx, &t, actor).await?;
    permit(&identity, &row.get::<String, _>("permission"))?;
    let action = m
        .actions
        .iter()
        .find(|a| a.name == row.get::<String, _>("action") && a.handler == "job")
        .ok_or(conflict("Job action was removed"))?;
    permit(
        &identity,
        action.permission.as_deref().unwrap_or("apps.manage"),
    )?;
    action_admission::additional(&identity, action)?;
    lock_package_write(&mut tx, &t, m).await?;
    let state: String = row.get("status");
    let out = if op == "job_claim" {
        if state != "queued" {
            return Err(conflict(
                "Job is already claimed/completed; uncertain outcomes require merchant review",
            ));
        }
        let lease = uid();
        if !veyndo_app_storage::jobs::claim(&mut tx, scope, id, &lease).await? {
            return Err(conflict(
                "Job is already claimed/completed; uncertain outcomes require merchant review",
            ));
        }
        json!({"id":id,"actor":row.get::<String,_>("actor"),"packageDigest":row.get::<String,_>("package_digest"),"action":row.get::<String,_>("action"),"input":row.get::<Value,_>("input"),"lease":lease,"leaseSeconds":60,"maximumMinutes":30,"checkpoint":row.get::<Option<Value>,_>("result").unwrap_or(json!({}))})
    } else {
        if !["running", "cancel_requested"].contains(&state.as_str())
            || !row.get::<Option<bool>, _>("valid").unwrap_or(false)
            || !row.get::<Option<bool>, _>("within_window").unwrap_or(false)
            || row.get::<Option<String>, _>("lease").as_deref() != v["lease"].as_str()
        {
            return Err(conflict(
                "Job lease expired or replaced; external outcome is uncertain",
            ));
        }
        let status = v["status"].as_str().unwrap_or("running");
        if state == "cancel_requested" && status == "running" {
            return Ok(json!({"cancelRequested":true}));
        }
        if !verified_kernel::job_completion_admissible(
            state == "cancel_requested",
            status == "succeeded",
        ) {
            return Err(conflict("Cancelled job cannot commit successful evidence"));
        }
        if status == "cancelled" && state != "cancel_requested" {
            return Err(conflict("No cancellation was requested"));
        }
        let progress = v["progress"]
            .as_i64()
            .unwrap_or(row.get::<i32, _>("progress") as i64);
        if progress < row.get::<i32, _>("progress") as i64 {
            return Err(bad("Job progress cannot move backwards"));
        }
        let message = message(v)?;
        job_evidence::save(
            a.rules,
            &mut tx,
            &t,
            job_evidence::Completion {
                service: h,
                creator: &identity,
                manifest: m,
                job: &row,
                report: v,
            },
        )
        .await?;
        let changed = veyndo_app_storage::progress::advance(
            &mut tx,
            scope,
            id,
            &veyndo_app_storage::progress::Progress {
                lease: v["lease"].as_str().unwrap(),
                status,
                progress: if status == "succeeded" {
                    100
                } else {
                    progress as i32
                },
                message: &message,
                result: &v["result"],
                checkpoint: v.get("checkpoint"),
            },
        )
        .await?;
        if !changed {
            return Err(conflict(
                "Job progress lease or state changed; external outcome is uncertain",
            ));
        }
        json!({"accepted":true,"cancelRequested":false})
    };
    tx.commit().await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::message;
    use serde_json::json;

    #[test]
    fn progress_messages_are_bounded_localized_text() {
        assert_eq!(message(&json!({})).unwrap(), json!({}));
        let text = json!({"message":{"en-GB":"Half done","de-DE":"Zur Hälfte fertig"}});
        assert_eq!(message(&text).unwrap(), text["message"]);
        assert!(message(&json!({"message":"Half done"})).is_err());
        assert!(message(&json!({"message":{"not a locale":"x"}})).is_err());
        assert!(message(&json!({"message":{"en-GB":"x".repeat(501)}})).is_err());
        assert!(message(&json!({"message":{"en-GB":7}})).is_err());
    }
}
