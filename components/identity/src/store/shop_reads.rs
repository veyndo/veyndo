//! Account and membership reads for the shop component's operator views: user totals, member counts, a shop's roster and an operator's account.
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// All merchant accounts.
pub async fn user_count(conn: &mut PgConnection) -> Result<i64> {
    Ok(
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM merchant_users"#)
            .fetch_one(conn)
            .await?,
    )
}

/// Active memberships per shop among `tenants`; shops without one are left out.
pub async fn members_by_shop(a: &State, tenants: &[String]) -> Result<Vec<(String, i64)>> {
    Ok(sqlx::query!(
        r#"SELECT tenant,count(*) AS "count!" FROM memberships WHERE tenant=ANY($1) AND active GROUP BY tenant"#,
        tenants
    )
    .fetch_all(&a.db)
    .await?
    .into_iter()
    .map(|r| (r.tenant, r.count))
    .collect())
}

/// A member of a shop's access roster.
pub struct RosterEntry {
    pub name: String,
    pub email: String,
    pub role: String,
    pub active: bool,
}

/// Up to a hundred members of a shop by name.
pub async fn roster(a: &State, tenant: &str) -> Result<Vec<RosterEntry>> {
    Ok(sqlx::query_as!(
        RosterEntry,
        "SELECT u.name,u.email,m.role,m.active FROM memberships m JOIN merchant_users u ON u.id=m.user_id WHERE m.tenant=$1 ORDER BY u.name LIMIT 100",
        tenant
    )
    .fetch_all(&a.db)
    .await?)
}

/// The ID, name and email of a merchant account.
pub struct AccountName {
    pub id: String,
    pub name: String,
    pub email: String,
}

/// The merchant account of a user.
pub async fn account(a: &State, user: &str) -> Result<AccountName> {
    Ok(sqlx::query_as!(
        AccountName,
        "SELECT id,name,email FROM merchant_users WHERE id=$1",
        user
    )
    .fetch_one(&a.db)
    .await?)
}
