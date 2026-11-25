//! Where rules and flows mention an ID, for the deletion guards of components below marketing.
use sqlx::PgConnection;
use veyndo_core::Result;

/// Rule conditions and flow definitions that contain the ID anywhere.
pub async fn mentions(tx: &mut PgConnection, tenant: &str, id: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT (SELECT count(*) FROM commerce_rules WHERE tenant=$1 AND jsonb_path_exists(condition,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text))) + (SELECT count(*) FROM commerce_flows WHERE tenant=$1 AND jsonb_path_exists(data,'$.** ? (@ == $id)',jsonb_build_object('id',$2::text))) AS "count!""#, tenant, id)
        .fetch_one(tx)
        .await?)
}
