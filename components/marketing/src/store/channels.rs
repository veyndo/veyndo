//! Sales channel rows and the product admission reads behind them: activity, parent activity, channel visibility and the company catalogue scope in one statement. Columns checked at compile time.
use serde_json::Value;
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

/// An admitted product and its parent.
pub struct Admitted {
    pub id: String,
    pub parent_id: Option<String>,
}

/// Those of `ids` that are active, under an active parent and not hidden in `channel`.
pub async fn visible(
    a: &State,
    tenant: &str,
    channel: &str,
    ids: &[String],
) -> Result<Vec<Admitted>> {
    Ok(sqlx::query_as!(Admitted, "SELECT p.id,p.parent_id FROM products p WHERE p.tenant=$1 AND p.id=ANY($3) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$2 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))",
        tenant, channel, ids)
    .fetch_all(&a.db)
    .await?)
}

/// The ids among `ids` that `visible` admits.
pub async fn visible_ids(
    a: &State,
    tenant: &str,
    ids: &[String],
    channel: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT p.id FROM products p WHERE p.tenant=$1 AND p.id=ANY($2) AND p.active AND (p.parent_id IS NULL OR EXISTS(SELECT 1 FROM products parent WHERE parent.tenant=p.tenant AND parent.id=p.parent_id AND parent.active)) AND NOT EXISTS(SELECT 1 FROM product_channel_visibility v WHERE v.tenant=p.tenant AND v.channel_id=$3 AND NOT v.visible AND (v.product_id=p.id OR v.product_id=p.parent_id))",
        tenant, ids, channel)
    .fetch_all(&a.db)
    .await?)
}

/// The ids among `ids` in the private catalogue of company unit `unit`, directly, through their parent or through a listed variant.
pub async fn in_unit_catalog(
    a: &State,
    tenant: &str,
    ids: &[String],
    unit: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!("SELECT p.id FROM products p WHERE p.tenant=$1 AND p.id=ANY($2) AND EXISTS(SELECT 1 FROM b2b_catalog b WHERE b.tenant=p.tenant AND b.unit_id=$3 AND (b.product_id=p.id OR b.product_id=p.parent_id OR (p.parent_id IS NULL AND EXISTS(SELECT 1 FROM products child WHERE child.tenant=p.tenant AND child.parent_id=p.id AND child.id=b.product_id))))",
        tenant, ids, unit)
    .fetch_all(&a.db)
    .await?)
}
