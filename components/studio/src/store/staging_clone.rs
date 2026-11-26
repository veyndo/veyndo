//! SQL of a staging clone: the sandbox shop row and its copied catalogue/configuration; customer, order and payment state never clones.
use super::staging_documents::Tx;
use serde_json::Value;
use veyndo_core::Result;

/// Manifests of the live shop's active apps, by ID.
pub async fn active_manifests(tx: &mut Tx<'_>, t: &str) -> Result<Vec<Value>> {
    veyndo_app_platform::store::staging::active_manifests(tx, t).await
}

/// Scopes app record row-level security to a shop for the rest of the transaction.
pub async fn scope_app_records(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query_scalar!("SELECT set_config('rac.tenant',$1,true)", t)
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}

/// Serializes environment creation and release for one live shop.
pub async fn lock_environments(tx: &mut Tx<'_>, t: &str) -> Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtextextended($1,16))", t)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// How many private environments the live shop has.
pub async fn environment_count(tx: &mut Tx<'_>, t: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM shop_environments WHERE live_tenant=$1"#,
        t
    )
    .fetch_one(&mut **tx)
    .await?)
}

/// Creates the sandbox shop and its environment row with the given baseline.
pub async fn insert_environment(
    tx: &mut Tx<'_>,
    live: &str,
    id: &str,
    name: &str,
    baseline: &Value,
) -> Result<()> {
    veyndo_core::store::sandbox::insert_shop(tx, id, name).await?;
    veyndo_core::store::environments_write::register(tx, id, live, name, baseline).await
}

/// Copies the live catalogue (products, translations, assets, categories) into the sandbox.
pub async fn copy_catalog(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    veyndo_catalog::store::staging::copy_catalog(tx, t, id).await
}

/// Copies settings, experience, receipts and the order workflow; app payment methods are dropped.
pub async fn copy_settings(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    veyndo_shop::store::staging::copy_settings(tx, t, id).await?;
    veyndo_intelligence::store::staging::copy_experience(tx, t, id).await?;
    veyndo_orders::store::staging::copy_receipts_and_workflow(tx, t, id).await?;
    // No external services or PSP configurations are registered for a sandbox tenant.
    veyndo_shop::store::staging::drop_app_payments(tx, id).await?;
    veyndo_marketing::store::staging::copy_promotions_flows_channels(tx, t, id).await
}

/// The sandbox's discovery/comparison policies and its company-limit extension.
pub async fn insert_defaults(tx: &mut Tx<'_>, id: &str, wat: &str, digest: &str) -> Result<()> {
    veyndo_intelligence::store::staging::insert_policies(tx, id).await?;
    veyndo_app_platform::store::staging::insert_extension(tx, id, wat, digest).await
}

/// Copies rules, channel visibility, company logos/overrides and channel overrides.
pub async fn copy_channel_configuration(tx: &mut Tx<'_>, t: &str, id: &str) -> Result<()> {
    veyndo_marketing::store::staging::copy_rules(tx, t, id).await?;
    veyndo_catalog::store::staging::copy_channel_visibility(tx, t, id).await?;
    veyndo_shop::store::staging::copy_company_and_overrides(tx, t, id).await
}

/// Replaces the environment's baseline.
pub async fn set_baseline(tx: &mut Tx<'_>, id: &str, baseline: &Value) -> Result<()> {
    veyndo_core::store::environments_write::set_baseline(tx, id, baseline).await
}

/// Marks the environment as a developer preview owned by one actor for one hour.
pub async fn mark_preview(tx: &mut Tx<'_>, id: &str, owner: &str) -> Result<()> {
    veyndo_core::store::environments_write::mark_preview(tx, id, owner).await
}
