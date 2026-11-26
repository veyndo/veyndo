//! Sales channel rows and the product admission reads behind them. Channel documents are checked columns; activity, channel visibility and the company catalogue scope come from the catalog and B2B stores.
use serde_json::Value;
use veyndo_catalog::store::rule_products::{self, Admitted};
use veyndo_catalog::store::unit_scope;
use veyndo_core::access::ChannelSnapshot;
use veyndo_core::{Result, State};

/// The channel document and revision.
pub async fn snapshot(a: &State, tenant: &str, id: &str) -> Result<Option<ChannelSnapshot>> {
    Ok(sqlx::query!(
        "SELECT data,revision FROM sales_channels WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(&a.db)
    .await?
    .map(|r| ChannelSnapshot {
        data: r.data,
        revision: r.revision,
    }))
}

/// The channel document.
pub async fn document(a: &State, tenant: &str, id: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM sales_channels WHERE tenant=$1 AND id=$2",
        tenant,
        id
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Those of `ids` that are active, under an active parent and not hidden in `channel`.
pub async fn visible(
    a: &State,
    tenant: &str,
    channel: &str,
    ids: &[String],
) -> Result<Vec<Admitted>> {
    rule_products::visible(a, tenant, channel, ids).await
}

/// The ids among `ids` that `visible` admits.
pub async fn visible_ids(
    a: &State,
    tenant: &str,
    ids: &[String],
    channel: &str,
) -> Result<Vec<String>> {
    rule_products::visible_ids(a, tenant, ids, channel).await
}

/// The ids among `ids` in the private catalogue of company unit `unit`, directly, through their parent or, for a main product, through a listed variant.
pub async fn in_unit_catalog(
    a: &State,
    tenant: &str,
    ids: &[String],
    unit: &str,
) -> Result<Vec<String>> {
    let mut tx = a.db.begin().await?;
    let listed = veyndo_b2b::store::catalog::listed(&mut tx, tenant, unit).await?;
    Ok(unit_scope::scope(&mut tx, tenant, ids, &listed)
        .await?
        .into_iter()
        .filter(|p| {
            listed.contains(&p.id)
                || p.parent_id
                    .as_ref()
                    .is_some_and(|parent| listed.contains(parent))
                || (p.parent_id.is_none() && p.variant_listed)
        })
        .map(|p| p.id)
        .collect())
}
