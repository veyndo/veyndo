//! Route that queues a document for vector indexing; retrieval itself is owned by the intelligence component.
use crate::{
    App, Error, Json, Path, RequestContext, Result, State, StatusCode, Value, auth, cognition,
    json, merchant,
};
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
