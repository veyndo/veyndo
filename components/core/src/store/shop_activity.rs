//! Whether a shop (or the live shop of a stage) may run background work: its status, the agent stop switch and the preview owner of a stage. Columns checked at compile time.
use crate::{Result, State};

/// The live shop's status and agent switch, with the stage's preview owner and whether that preview is still valid.
pub struct ShopActivity {
    pub status: String,
    pub agents_stopped: bool,
    pub preview_owner: Option<String>,
    pub valid: Option<bool>,
}

/// The activity of shop `tenant`, resolved through its live shop when it is a stage.
pub async fn activity(a: &State, tenant: &str) -> Result<ShopActivity> {
    Ok(sqlx::query_as!(ShopActivity, "SELECT t.status,t.agents_stopped,e.preview_owner,e.preview_until>now() AS valid FROM tenants t LEFT JOIN shop_environments e ON e.tenant=$1 WHERE t.id=coalesce(e.live_tenant,$1)", tenant)
        .fetch_one(&a.db)
        .await?)
}

/// Those of `tenants` whose live shop (the shop itself, or the live shop of a stage) is active.
pub async fn active(a: &State, tenants: &[String]) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT s.id FROM unnest($1::text[]) AS s(id) WHERE EXISTS(SELECT 1 FROM tenants t WHERE t.id=coalesce((SELECT live_tenant FROM shop_environments WHERE tenant=s.id),s.id) AND t.status='active')",
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .flatten()
    .collect())
}
