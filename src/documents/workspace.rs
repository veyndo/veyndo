//! Permission-filtered knowledge census, cursor source inventory and activity; totals never masquerade as sampled graph counts.
use super::*;
#[derive(Default, serde::Deserialize)]
pub(crate) struct WorkspaceQuery {
    pub after: Option<String>,
    pub query: Option<String>,
    pub archived: Option<bool>,
    pub kind: Option<String>,
}
pub(crate) async fn workspace(
    State(a): State<App>,
    h: RequestContext,
    axum::extract::Query(q): axum::extract::Query<WorkspaceQuery>,
) -> Result<Json<Value>> {
    let t = merchant(&a, &h)?;
    auth::permit(&h, "knowledge.read")?;
    let query = q.query.as_deref().unwrap_or("");
    if query.len() > 200 || q.after.as_ref().is_some_and(|s| s.len() > 100) {
        return Err(bad("Knowledge filter too long"));
    }
    let locale = language_context(&a, &h).await?.0;
    let (settings, _) = commerce::config(&a, &t).await?;
    let (totals, rows, next, activity) = veyndo_intelligence::store::documents::inventory(
        &a,
        &t,
        veyndo_intelligence::store::documents::Inventory {
            after: q.after.as_deref().unwrap_or(""),
            query,
            archived: q.archived.unwrap_or(false),
            kind: q.kind.as_deref().unwrap_or(""),
            locale: &locale,
            main_locale: &settings.main_locale,
        },
    )
    .await?;
    Ok(Json(
        json!({"totals":totals,"sources":rows.into_iter().take(50).collect::<Vec<_>>(),"next":next,"activity":activity,"mainLocale":settings.main_locale,"locales":settings.locales,"canWrite":auth::allowed(&h,"knowledge.write"),"sampleLimits":{"sources":50,"activity":20,"graphNeeds":48,"graphPairs":48,"observations":24},"modelWeightsUpdated":false,"causalUpliftProven":false}),
    ))
}
