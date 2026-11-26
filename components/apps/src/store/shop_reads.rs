//! App reads for the shop component's operator views: active packages and evidence sources per shop.
use veyndo_core::{Result, State};

/// Active app packages per shop among `tenants`; shops without one are left out.
pub async fn active_by_shop(a: &State, tenants: &[String]) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM app_packages WHERE tenant=ANY($1) AND active GROUP BY tenant"#,
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}

/// Evidence sources per shop among `tenants`; shops without one are left out.
pub async fn sources_by_shop(a: &State, tenants: &[String]) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM app_evidence WHERE tenant=ANY($1) GROUP BY tenant"#,
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}
