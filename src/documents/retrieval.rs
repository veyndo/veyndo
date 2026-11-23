//! Bounded lexical/vector source retrieval; public questions use only explicitly published documents.
use super::*;
use veyndo_intelligence::store::documents as store;
pub(crate) async fn search_in(
    a: &App,
    t: &str,
    product: Option<&str>,
    query: &str,
    public: bool,
    locale: &str,
    semantic: bool,
) -> Result<Value> {
    let q = veyndo_intelligence::service::retrieval::SourceQuery {
        product,
        query,
        public,
        locale,
        semantic,
    };
    veyndo_intelligence::service::retrieval::search_in(a.models(), t, q).await
}
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
