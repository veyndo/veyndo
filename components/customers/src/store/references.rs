//! Customer reads for the shop component's operator views and settings guards: counts and customer group references.
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// Customers of all shops outside `excluded`.
pub async fn count_outside(conn: &mut PgConnection, excluded: &[String]) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM customers WHERE NOT (tenant=ANY($1))"#,
        excluded
    )
    .fetch_one(conn)
    .await?)
}

/// Customers per shop among `tenants`; shops without customers are left out.
pub async fn count_by_shop(a: &State, tenants: &[String]) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM customers WHERE tenant=ANY($1) GROUP BY tenant"#,
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}

/// Whether a customer of the shop belongs to a group outside `groups`.
pub async fn outside_groups(tx: &mut PgConnection, t: &str, groups: &[String]) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND NOT(group_name=ANY($2))) AS "used!""#,
        t,
        groups
    )
    .fetch_one(tx)
    .await?)
}
