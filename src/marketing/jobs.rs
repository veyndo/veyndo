//! Flow execution summaries are owned by the marketing component; the reader's scopes decide how much it sees.
use super::*;
pub(super) async fn values(a: &App, t: &str, h: &RequestContext) -> Result<Value> {
    let full = auth::SCOPES.iter().all(|s| auth::allowed(h, s));
    veyndo_marketing::store::flow_job_summaries::values(a, t, full).await
}
pub(super) async fn list(State(a): State<App>, h: RequestContext) -> Result<Json<Value>> {
    auth::permit(&h, "automation.read")?;
    Ok(Json(json!({"jobs":values(&a,&merchant(&a,&h)?,&h).await?})))
}
