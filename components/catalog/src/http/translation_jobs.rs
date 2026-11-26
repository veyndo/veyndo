//! Resumable translation jobs: enqueueing a job for an enabled target language, the job list, its paginated drafts and resume/cancel controls.
use crate::service::translation_apply::admit_selection;
use crate::store::translation_jobs::{self, NewJob};
use axum::Json;
use axum::extract::{Path, Query, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use veyndo_core::document::{lenient, view};
use veyndo_core::{RequestContext, Result, State, bad, conflict};
use veyndo_identity::{merchant, permit};

/// Both explicit settings commands and approved merchant tasks enqueue the same durable draft owner; the target must be an enabled language other than the main one.
pub async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    id: &str,
    languages: (&str, &[String]),
    request: (&str, &Value, bool, Option<&[String]>),
) -> Result<()> {
    let (target, choice, overwrite, products) = request;
    let (main_locale, locales) = languages;
    admit_selection(tx, t, products).await?;
    if !locales.iter().any(|l| l == target) || target == main_locale {
        return Err(bad("Select an enabled translation target"));
    }
    let (total, highwater) = translation_jobs::selection_size(tx, t, products).await?;
    let job = NewJob {
        id,
        source_locale: main_locale,
        target_locale: target,
        choice,
        overwrite,
        total,
        highwater,
        products,
    };
    translation_jobs::insert(tx, t, job).await?;
    Ok(())
}

/// `GET /api/merchant/translations`: the latest translation jobs.
pub async fn list(Shared(a): Shared<State>, h: RequestContext) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let t = merchant(&h)?;
    let rows = translation_jobs::latest(&a, &t).await?;
    Ok(Json(json!({"jobs":rows})))
}
/// `GET /api/merchant/translations/{id}`: a job with its status counts and a page of drafts.
pub async fn detail(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    permit(&h, "catalog.read")?;
    let t = merchant(&h)?;
    let job = translation_jobs::document(&a, &t, &id)
        .await?
        .ok_or(bad("Translation job unavailable"))?;
    let cursor = q.get("cursor").map(String::as_str).unwrap_or("");
    let items = translation_jobs::draft_page(&a, &t, &id, cursor).await?;
    let counts = translation_jobs::status_counts(&a, &t, &id).await?;
    Ok(Json(
        json!({"job":job,"counts":counts,"items":items.iter().take(50).collect::<Vec<_>>(),"nextCursor":if items.len()>50 {items[49]["product_id"].clone()}else{Value::Null}}),
    ))
}
/// The JSON fields `control` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ControlInput {
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
}

/// `PUT /api/merchant/translations/{id}`: resume or cancel a job that no task controls.
pub async fn control(
    Shared(a): Shared<State>,
    h: RequestContext,
    Path(id): Path<String>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: ControlInput = view(&v);
    permit(&h, "catalog")?;
    let t = merchant(&h)?;
    let status = match input.action.as_deref() {
        Some("resume") => "queued",
        Some("cancel") => "cancelled",
        _ => return Err(bad("Invalid translation action")),
    };
    if !translation_jobs::control(&a, &t, &id, status).await? {
        return Err(conflict(
            "Translation is complete or controlled by its original task; use the task continuation controls",
        ));
    }
    Ok(Json(json!({"status":status})))
}
