//! Authorized translation job creation, progress, paginated drafts and resume/cancel controls.
use super::selection;
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, commerce};
use axum::Json;
use axum::extract::State;
use serde_json::Value;
use serde_json::json;
use uuid::Uuid;
use veyndo_cognition::inference::Choice;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::request_context::RequestContext;
pub(crate) async fn create(
    State(a): State<App>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    auth::permit(&h, "catalog")?;
    let t = merchant(&a, &h)?;
    let (settings, _) = commerce::config(&a, &t).await?;
    let choice: Choice = serde_json::from_value(
        v.get("inference")
            .cloned()
            .unwrap_or(json!({"provider":"ollama","model":null})),
    )
    .map_err(|_| bad("Invalid translation provider"))?;
    let products = selection::parse(v.get("productIds"))?;
    veyndo_studio::service::work_execution::admit_provider(&a.models.inference, &choice).await?;
    let mut tx = a.db.begin().await?;
    let id = Uuid::new_v4().to_string();
    veyndo_studio::service::work_execution::enqueue_translation(
        &mut tx,
        &t,
        &id,
        &settings,
        (
            v["targetLocale"]
                .as_str()
                .ok_or(bad("Target language required"))?,
            &choice,
            v["overwrite"].as_bool().unwrap_or(false),
            products.as_deref(),
        ),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id,"status":"queued"})))
}
pub(crate) use veyndo_catalog::http::translation_jobs::{control, detail, list};
