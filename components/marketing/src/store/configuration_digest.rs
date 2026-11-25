//! The configuration digest of a shop: every installed app, rule, flow, promotion and channel revision in one statement, so a scenario can tell whether the configuration changed.
use sqlx::PgExecutor;
use veyndo_core::Result;

/// A digest of every installed app, rule, flow, promotion and channel revision of the shop.
pub async fn digest(e: impl PgExecutor<'_>, tenant: &str) -> Result<String> {
    Ok(sqlx::query_scalar!(r#"SELECT md5(coalesce(string_agg(stamp,'|' ORDER BY stamp),'')) AS "digest!" FROM (SELECT 'app:'||id||':'||digest||':'||revision||':'||active AS stamp FROM app_packages WHERE tenant=$1 UNION ALL SELECT 'rule:'||id||':'||revision FROM commerce_rules WHERE tenant=$1 UNION ALL SELECT 'flow:'||id||':'||revision FROM commerce_flows WHERE tenant=$1 UNION ALL SELECT 'promotion:'||id||':'||revision FROM commerce_promotions WHERE tenant=$1 UNION ALL SELECT 'channel:'||id||':'||revision FROM sales_channels WHERE tenant=$1) versions"#,
        tenant).fetch_one(e).await?)
}
