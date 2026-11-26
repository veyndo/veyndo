//! The job list, detail and control of an app.
use crate::domain::{action_admission, approval, persistence};
use crate::store::job_control;
use crate::store::packages::package;
use axum::Json;
use axum::extract::{Path, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, conflict};
use veyndo_identity::{merchant, permit};

/// Paging of the job list.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobPage {
    limit: Option<i64>,
    after: Option<String>,
    group: Option<String>,
}
/// A page of the jobs of an app, projected to what the actor may see.
pub async fn list(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<JobPage>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &id, false).await?;
    let limit = q.limit.unwrap_or(100);
    let mut tx = a.db.begin().await?;
    let page = veyndo_app_storage::job_history::list(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: &t,
            app: &id,
        },
        &veyndo_app_storage::job_history::Query {
            limit,
            after: q.after.as_deref(),
            group: q.group.as_deref(),
        },
    )
    .await
    .map_err(persistence::error)?;
    tx.commit().await?;
    let mut rows = page.rows;
    veyndo_app_storage::job_projection::history(&action_admission::Current(&h), &m, &mut rows);
    Ok(Json(
        json!({"jobs":rows,"hasMore":page.has_more,"nextCursor":page.next_cursor,"limit":limit}),
    ))
}
/// One job for its creator or an app manager, under the current package.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((app, id)): Path<(String, String)>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &app, true).await?;
    let mut tx = a.db.begin().await?;
    let row = veyndo_app_storage::job_history::detail(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: &t,
            app: &app,
        },
        &id,
    )
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "App job not found".into()))?;
    if h.principal.user.as_deref() != Some(row.get::<String, _>("actor").as_str())
        && permit(&h, "apps.manage").is_err()
    {
        return Err(Error(StatusCode::NOT_FOUND, "App job not found".into()));
    }
    if row.get::<String, _>("package_digest") != approval::canonical_digest(&m) {
        return Err(conflict("Job package changed; review its archived result"));
    }
    let name: String = row.get("action");
    let action = m
        .actions
        .iter()
        .find(|a| a.name == name && a.handler == "job")
        .ok_or(conflict("Job action removed"))?;
    permit(&h, action.permission.as_deref().unwrap_or("apps.manage"))?;
    action_admission::additional(&h, action)?;
    tx.commit().await?;
    Ok(Json(row.get("data")))
}
/// Cancels or retries a job through the shared control transaction.
pub async fn control(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path((app, id)): Path<(String, String)>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    let m = package(&a, &t, &app, true).await?;
    let mut tx = a.db.begin().await?;
    let digest = approval::canonical_digest(&m);
    let result = veyndo_app_storage::job_control::control(
        &mut tx,
        &veyndo_app_storage::job_control::Context {
            scope: veyndo_app_storage::Scope {
                tenant: &t,
                app: &app,
            },
            actor: h.principal.user.as_deref(),
            package_digest: &digest,
        },
        &id,
        &v,
        &job_control::Admission {
            context: &h,
            manifest: &m,
        },
    )
    .await
    .map_err(job_control::error)?;
    tx.commit().await?;
    Ok(Json(result))
}
