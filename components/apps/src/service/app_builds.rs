//! App Studio builds: immutable development versions saved after admission, listed, archived, and staged into an environment the shop owns.
use crate::domain::app_generation::default_actions;
use crate::domain::consent::actor_permissions;
use crate::domain::native_lifecycle::validate_locale;
use crate::domain::{build_admission, design_context, generation_bindings};
use crate::ports::{CommerceInstall, Platform, RecordRules};
use crate::service::package_verification::verify_package;
use crate::store::package_install::install_tx;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgRow;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::Scope;
use veyndo_app_storage::build_versions as shared;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{owned_stage, parent};
use veyndo_core::{
    Error, RequestContext, Result, State, StatusCode, bad, conflict, process_config, uid,
};
use veyndo_identity::{merchant, permit};

/// The live shop of a merchant request; environment management never runs from inside a stage.
pub async fn live(a: &State, h: &RequestContext) -> Result<String> {
    let t = merchant(h)?;
    if parent(a, &t).await?.is_some() {
        return Err(bad("Use the live workspace for environment management"));
    }
    Ok(t)
}

/// The storage error of a build or draft as the error a caller sees.
pub fn error(e: veyndo_app_storage::Error) -> Error {
    match e {
        veyndo_app_storage::Error::Database(e) => e.into(),
        veyndo_app_storage::Error::Invalid(s) => bad(s),
        veyndo_app_storage::Error::Conflict(s) => conflict(s),
        veyndo_app_storage::Error::Quota(s) => Error(StatusCode::TOO_MANY_REQUESTS, s.into()),
    }
}

fn stage_error(e: shared::StageError<Error>) -> Error {
    match e {
        shared::StageError::Database(e) => e.into(),
        shared::StageError::Unavailable => Error(StatusCode::NOT_FOUND, "Build unavailable".into()),
        shared::StageError::Conflict(s) => conflict(s),
        shared::StageError::Host(e) => e,
    }
}

/// Admission of a stored build into an environment: current ownership, current package rules and the rights of the actor.
struct Admission<'a> {
    live: &'a str,
    actor: &'a RequestContext,
    rules: &'a dyn RecordRules,
    commerce: &'a dyn CommerceInstall,
}
impl<'db> shared::StageAdmission<'db> for Admission<'_> {
    type Error = Error;
    fn install<'a>(
        &'a self,
        tx: &'a mut sqlx::Transaction<'db, sqlx::Postgres>,
        environment: &'a str,
        manifest: &'a Value,
    ) -> shared::StageFuture<'a, Self::Error>
    where
        'db: 'a,
    {
        Box::pin(async move {
            owned_stage(&mut **tx, self.live, environment).await?;
            let m: Manifest =
                serde_json::from_value(manifest.clone()).map_err(|_| bad("Invalid app"))?;
            build_admission::validate(&m, &process_config::get().publishers)?;
            actor_permissions(self.actor, &m)?;
            install_tx(self.rules, self.commerce, tx, environment, m).await
        })
    }
}

/// Where a build came from and what the shop requires of it.
pub struct Origin<'a> {
    /// The provider that produced the manifest, or `import`.
    pub provider: &'a str,
    /// The model that produced it, when one did.
    pub model: Option<&'a str>,
    /// Design evidence kept with the prompt.
    pub evidence: Option<&'a Value>,
    /// The shop main language every label and the summary must carry.
    pub main: &'a str,
    /// The actions a payment provider package gets when it declares none.
    pub payment: &'a [Value],
}

/// The input of an App Studio build operation: save, archive or stage.
#[derive(Deserialize, Default)]
#[serde(default)]
struct BuildInput {
    #[serde(deserialize_with = "lenient")]
    environment: Option<String>,
    manifest: Value,
    #[serde(deserialize_with = "lenient")]
    prompt: Option<String>,
    summary: Value,
    #[serde(deserialize_with = "lenient")]
    app: Option<String>,
    #[serde(deserialize_with = "lenient")]
    archived: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    digest: Value,
}

/// Validates a manifest for a private environment of the shop and stores it as an immutable development version.
pub async fn save(a: &State, t: &str, v: &Value, origin: &Origin<'_>) -> Result<Value> {
    let input: BuildInput = view(v);
    let stage = input
        .environment
        .as_deref()
        .ok_or(bad("Private environment required"))?;
    owned_stage(&a.db, t, stage).await?;
    let mut value = input.manifest.clone();
    default_actions(&mut value, origin.payment)?;
    generation_bindings::complete(&mut value)?;
    let m: Manifest = serde_json::from_value(value).map_err(|e| bad(e.to_string()))?;
    build_admission::validate(&m, &process_config::get().publishers)?;
    let prompt = input
        .prompt
        .as_deref()
        .filter(|s| !s.trim().is_empty() && s.len() <= 8000)
        .ok_or(bad("Prompt required, maximum 8000 bytes"))?;
    validate_locale(&m, origin.main)?;
    build_admission::main_language(&m, &input.summary, origin.main)?;
    verify_package(&m).await?;
    let manifest = json!(m);
    let id = uid();
    let retained = design_context::stored(prompt, origin.evidence);
    let mut tx = a.db.begin().await?;
    let digest = shared::save(
        &mut tx,
        &shared::Build {
            scope: Scope {
                tenant: t,
                app: &m.id,
            },
            id: &id,
            environment: stage,
            version: &m.version,
            manifest: &manifest,
            prompt: &retained,
            provider: origin.provider,
            model: origin.model,
            summary: &input.summary,
        },
    )
    .await
    .map_err(error)?;
    tx.commit().await?;
    Ok(
        json!({"id":id,"app":m.id,"version":m.version,"digest":digest,"manifest":manifest,"summary":input.summary,"state":"draft","environment":stage,"designEvidence":origin.evidence}),
    )
}

fn build(r: &PgRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"environment":r.get::<String,_>("environment"),"app":r.get::<String,_>("app"),"version":r.get::<String,_>("version"),"digest":r.get::<String,_>("digest"),"manifest":r.get::<Value,_>("manifest"),"summary":r.get::<Value,_>("summary"),"state":r.get::<String,_>("state"),"provider":r.get::<String,_>("provider"),"model":r.get::<Option<String>,_>("model"),"designEvidence":design_context::restored(&r.get::<String,_>("prompt"))})
}

/// The current and the archived builds of a shop, newest first.
pub async fn list(a: &State, t: &str) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut tx = a.db.begin().await?;
    let rows = shared::list(&mut tx, t, false).await?;
    let trash = shared::list(&mut tx, t, true).await?;
    tx.commit().await?;
    Ok((
        rows.iter().map(build).collect(),
        trash.iter().map(build).collect(),
    ))
}

/// Moves an App Studio project to the trash or back; installed packages and their records are untouched.
pub async fn archive(a: &State, h: &RequestContext, v: &Value) -> Result<Value> {
    permit(h, "developer.manage")?;
    let t = live(a, h).await?;
    let input: BuildInput = view(v);
    let app = input
        .app
        .as_deref()
        .filter(|s| !s.is_empty() && s.len() <= 32)
        .ok_or(bad("App required"))?;
    let archived = input.archived.ok_or(bad("Archived boolean required"))?;
    if input.approve != Some(true) {
        return Err(bad("Approve the App Studio removal/restore first"));
    }
    let mut tx = a.db.begin().await?;
    let n = shared::archive(&mut tx, Scope { tenant: &t, app }, archived).await?;
    if n == 0 {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "App Studio project unavailable in this state".into(),
        ));
    }
    tx.commit().await?;
    Ok(json!({"app":app,"archived":archived,"versions":n}))
}

/// Installs a reviewed build into its environment after the merchant approved exactly this digest.
pub async fn stage(a: &Platform, h: &RequestContext, id: &str, v: &Value) -> Result<Value> {
    permit(h, "developer.manage")?;
    let t = live(a, h).await?;
    let input: BuildInput = view(v);
    if input.approve != Some(true) {
        return Err(bad("Approve the reviewed build first"));
    }
    let mut tx = a.db.begin_privileged().await?;
    let result = shared::stage(
        &mut tx,
        &t,
        id,
        &input.digest,
        &Admission {
            live: &t,
            actor: h,
            rules: a.rules,
            commerce: a.commerce,
        },
    )
    .await
    .map_err(stage_error)?;
    tx.commit().await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{error, stage_error};
    use veyndo_app_storage::build_versions::StageError;
    use veyndo_core::{Error, StatusCode};

    #[test]
    fn storage_errors_keep_their_meaning_for_the_caller() {
        assert_eq!(
            error(veyndo_app_storage::Error::Invalid("bad")).0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            error(veyndo_app_storage::Error::Conflict("stale")).0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            error(veyndo_app_storage::Error::Quota("full")).0,
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            stage_error(StageError::Unavailable).0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            stage_error(StageError::Conflict("changed")).0,
            StatusCode::CONFLICT
        );
        let host = Error(StatusCode::FORBIDDEN, "Missing right".into());
        assert_eq!(stage_error(StageError::Host(host)).0, StatusCode::FORBIDDEN);
    }
}
