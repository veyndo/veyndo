//! Source-bound claim lifecycle: permissions and the shop languages here, the ledger in the intelligence component.
use super::*;
use veyndo_intelligence::store::evidence as store;
pub(crate) use veyndo_intelligence::store::evidence::{NODE_TYPES, claims};
pub(crate) async fn propose(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "knowledge.write")?;
    let mut tx = a.db.begin().await?;
    let result = propose_in(a, &mut tx, &t, h, v).await?;
    tx.commit().await?;
    Ok(result)
}
pub(crate) async fn propose_in(
    a: &App,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    h: &RequestContext,
    v: &Value,
) -> Result<Value> {
    let (settings, _) = commerce::config(a, t).await?;
    store::propose_in(
        tx,
        t,
        header(h, "x-rac-user").unwrap_or("integration"),
        v,
        &settings.main_locale,
        &settings.locales,
    )
    .await
}
pub(crate) async fn decide(a: &App, h: &RequestContext, v: &Value) -> Result<Value> {
    let t = merchant(a, h)?;
    auth::permit(h, "knowledge.write")?;
    store::decide(a, &t, header(h, "x-rac-user"), v).await
}
