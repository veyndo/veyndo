//! App Studio transport for private autosaves, project archive and staging a reviewed build.
use crate::ports::Platform;
use crate::service::{app_builds, app_preview};
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde_json::{Value, json};
use veyndo_app_storage::drafts as shared;
use veyndo_core::shop_scope::header;
use veyndo_core::{RequestContext, Result, State, hash};
use veyndo_identity::merchant;

/// The owner of a draft: the signed-in user, or a digest of the presented credential.
fn actor(h: &RequestContext) -> String {
    h.principal
        .user
        .clone()
        .unwrap_or_else(|| hash(header(h, "authorization").unwrap_or("")))
}

/// Lists the private autosaves of the caller.
pub async fn drafts(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let owner = actor(&h);
    let mut tx = a.db.begin().await?;
    let rows = shared::list(
        &mut tx,
        shared::Owner {
            tenant: &t,
            actor: &owner,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"drafts":rows})))
}

/// Saves one autosave under optimistic revision control; nothing is installed or published.
pub async fn save_draft(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let owner = actor(&h);
    let mut tx = a.db.begin().await?;
    let revision = shared::save(
        &mut tx,
        shared::Owner {
            tenant: &t,
            actor: &owner,
        },
        &shared::Draft {
            id: &id,
            manifest: &v["manifest"],
            environment: v["environment"].as_str(),
            revision: v["revision"].as_i64(),
        },
        &OwnedEnvironment,
    )
    .await
    .map_err(app_builds::error)?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"revision":revision})))
}

/// Deletes one autosave of the caller.
pub async fn remove_draft(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let owner = actor(&h);
    let mut tx = a.db.begin().await?;
    shared::remove(
        &mut tx,
        shared::Owner {
            tenant: &t,
            actor: &owner,
        },
        &id,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"deleted":true})))
}

/// The relation between a shop and its environments, read on the scoped connection of the save.
struct OwnedEnvironment;
impl shared::EnvironmentAdmission for OwnedEnvironment {
    fn owned<'a>(
        &'a self,
        tx: &'a mut sqlx::PgConnection,
        tenant: &'a str,
        environment: &'a str,
    ) -> shared::AdmissionFuture<'a> {
        Box::pin(async move {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM shop_environments WHERE live_tenant=$1 AND tenant=$2)",
            )
            .bind(tenant)
            .bind(environment)
            .fetch_one(tx)
            .await
        })
    }
}

/// Moves an App Studio project to the trash after explicit approval.
pub async fn remove(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(app): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let request = json!({"app":app,"archived":true,"approve":v["approve"]});
    Ok(Json(app_builds::archive(&a, &h, &request).await?))
}

/// Restores an App Studio project from the trash after explicit approval.
pub async fn restore(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(app): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let request = json!({"app":app,"archived":false,"approve":v["approve"]});
    Ok(Json(app_builds::archive(&a, &h, &request).await?))
}

/// Installs a reviewed build into its private environment.
pub async fn stage(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(app_builds::stage(&a, &h, &id, &v).await?))
}

/// Runs the draft in the personal preview environment of the caller.
pub async fn preview(
    Shared(a): Shared<Platform>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    Ok(Json(app_preview::run(&a, &h, v).await?))
}

#[cfg(test)]
mod tests {
    use super::actor;
    use veyndo_core::{RequestContext, hash};

    #[test]
    fn a_draft_belongs_to_the_presented_credential_when_no_user_is_signed_in() {
        let mut h = RequestContext::new();
        assert_eq!(actor(&h), hash(""));
        h.insert("authorization", "Bearer integration".parse().unwrap());
        assert_eq!(actor(&h), hash("Bearer integration"));
    }
}
