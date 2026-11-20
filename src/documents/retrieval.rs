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
    let (settings, _) = commerce::config(a, t).await?;
    let model = env::var("EMBEDDING_MODEL").unwrap_or("qwen3-embedding:0.6b".into());
    let indexed = store::indexed(a, t, &model, public, product).await?;
    let vector = if indexed && semantic {
        cognition::indexing::query_embedding(a, t, query).await
    } else {
        None
    };
    store::search(
        a,
        t,
        store::Search {
            product,
            query,
            public,
            locale,
            main_locale: &settings.main_locale,
            model: &model,
        },
        vector,
    )
    .await
}
pub(crate) use veyndo_intelligence::store::documents::sources_current;
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
