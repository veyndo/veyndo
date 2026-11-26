//! Route that queues a document for vector indexing; retrieval itself is owned by the intelligence component.
use crate::foundation::App;
use crate::foundation::merchant;
use crate::{auth, cognition};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;
use veyndo_core::Error;
use veyndo_core::Result;
use veyndo_core::request_context::RequestContext;
use veyndo_intelligence::store::documents as store;
pub(crate) async fn index(
    State(a): State<App>,
    h: RequestContext,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.write")?;
    let exists = store::exists(&a, &t, &id).await?;
    if !exists {
        return Err(Error(StatusCode::NOT_FOUND, "Document not found".into()));
    }
    let queued = cognition::indexing::enqueue(&a, &t, Some(&id)).await?;
    Ok(Json(
        json!({"queued":queued,"indexed":0,"model":cognition::indexing::model(),"asynchronous":true}),
    ))
}
