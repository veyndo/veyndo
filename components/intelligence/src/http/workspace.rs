//! The knowledge workspace: a permission-filtered census, the source inventory by cursor and recent activity. Totals never pose as sampled graph counts.
use crate::store::documents::{Inventory, inventory};
use axum::Json;
use axum::extract::{Query, State as Shared};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::{RequestContext, Result, State, bad};
use veyndo_identity::domain::permissions::allowed;
use veyndo_identity::{merchant, permit};
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::config;

/// The filters of a workspace page.
#[derive(Default, Deserialize)]
pub struct WorkspaceQuery {
    pub after: Option<String>,
    pub query: Option<String>,
    pub archived: Option<bool>,
    pub kind: Option<String>,
}

/// Rejects filters beyond their bounds.
pub fn bounded(q: &WorkspaceQuery) -> Result<()> {
    if q.query.as_ref().is_some_and(|s| s.len() > 200)
        || q.after.as_ref().is_some_and(|s| s.len() > 100)
    {
        return Err(bad("Knowledge filter too long"));
    }
    Ok(())
}

/// One page of the knowledge workspace.
pub async fn workspace(
    Shared(a): Shared<State>,
    h: RequestContext,
    Query(q): Query<WorkspaceQuery>,
) -> Result<Json<Value>> {
    let t = merchant(&h)?;
    permit(&h, "knowledge.read")?;
    bounded(&q)?;
    let query = q.query.as_deref().unwrap_or("");
    let locale = language_context(&a, &h).await?.0;
    let (settings, _) = config(&a, &t).await?;
    let (totals, rows, next, activity) = inventory(
        &a,
        &t,
        Inventory {
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
        json!({"totals":totals,"sources":rows.into_iter().take(50).collect::<Vec<_>>(),"next":next,"activity":activity,"mainLocale":settings.main_locale,"locales":settings.locales,"canWrite":allowed(&h,"knowledge.write"),"sampleLimits":{"sources":50,"activity":20,"graphNeeds":48,"graphPairs":48,"observations":24},"modelWeightsUpdated":false,"causalUpliftProven":false}),
    ))
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceQuery, bounded};

    #[test]
    fn workspace_filters_are_bounded() {
        assert!(bounded(&WorkspaceQuery::default()).is_ok());
        let long = WorkspaceQuery {
            query: Some("x".repeat(201)),
            ..Default::default()
        };
        assert!(bounded(&long).is_err());
        let cursor = WorkspaceQuery {
            after: Some("x".repeat(101)),
            ..Default::default()
        };
        assert!(bounded(&cursor).is_err());
    }
}
