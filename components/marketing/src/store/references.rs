//! Where rules, flows and promotions mention an ID or which documents they hold, for the guards of components below marketing.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Rule conditions and flow definitions that contain the ID anywhere.
pub async fn mentions(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT (SELECT count(*) FROM commerce_rules WHERE tenant=$1 AND jsonb_path_exists(condition,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text))) + (SELECT count(*) FROM commerce_flows WHERE tenant=$1 AND jsonb_path_exists(data,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text))) AS "count!""#, tenant, id)
        .fetch_one(tx)
        .await?)
}

/// The rule conditions, flow definitions and promotion definitions of the shop, in that order.
pub async fn documents(tx: &mut PgConnection, tenant: &str) -> Result<Vec<serde_json::Value>> {
    Ok(sqlx::query_scalar!(r#"SELECT condition AS "data!" FROM commerce_rules WHERE tenant=$1 UNION ALL SELECT data FROM commerce_flows WHERE tenant=$1 UNION ALL SELECT data FROM commerce_promotions WHERE tenant=$1"#, tenant)
        .fetch_all(tx)
        .await?)
}
