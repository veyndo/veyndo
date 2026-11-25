//! Customer rows as rules and flows read and change them: the rule facts of an account, the locked automation data and group, the flow's group/status change with session revocation, the revision-bound automation write and the channel's customer count. Marketing reaches them through its records port. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The customer fields customer rules read, with calendar age and login distances in days.
pub struct RuleCustomer {
    pub active: bool,
    pub email: String,
    pub group_name: String,
    pub customer_number: String,
    pub profile: Value,
    pub automation: Value,
    pub company: Option<String>,
    pub age: Option<f64>,
    pub first_days: Option<f64>,
    pub last_days: Option<f64>,
}

/// The rule facts of customer `id`; none for a guest or an unknown id.
pub async fn rule_facts(
    conn: &mut PgConnection,
    tenant: &str,
    id: Option<&str>,
) -> Result<Option<RuleCustomer>> {
    Ok(sqlx::query_as!(RuleCustomer, "SELECT active,email,group_name,customer_number,profile,automation,company,extract(year FROM age(current_date,(profile->>'birthday')::date))::double precision AS age,abs(current_date-first_login::date)::double precision AS first_days,abs(current_date-last_login::date)::double precision AS last_days FROM customers WHERE tenant=$1 AND id=$2", tenant, id)
        .fetch_optional(conn)
        .await?)
}

/// The group of customer `id`, locking the customer row.
pub async fn lock_group(conn: &mut PgConnection, tenant: &str, id: Option<&str>) -> Result<String> {
    Ok(sqlx::query_scalar!(
        "SELECT group_name FROM customers WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_one(conn)
    .await?)
}

/// The automation data of customer `id`, locking the customer row; none for an unknown id.
pub async fn lock_automation(
    conn: &mut PgConnection,
    tenant: &str,
    id: Option<&str>,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT automation FROM customers WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// Changes the group and/or active flag of customer `id` and advances its revision; the number of rows changed.
pub async fn set_group_and_status(
    conn: &mut PgConnection,
    (group, active): (Option<&str>, Option<bool>),
    tenant: &str,
    id: Option<&str>,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE customers SET group_name=COALESCE($1,group_name),active=COALESCE($2,active),revision=revision+1 WHERE tenant=$3 AND id=$4", group, active, tenant, id).execute(conn).await?.rows_affected())
}

/// Ends every session of customer `id`.
pub async fn end_sessions(conn: &mut PgConnection, tenant: &str, id: Option<&str>) -> Result<()> {
    sqlx::query!("DELETE FROM customer_sessions WHERE tenant=$1 AND email=(SELECT email FROM customers WHERE tenant=$1 AND id=$2)", tenant, id).execute(conn).await?;
    Ok(())
}

/// Stores the automation data of customer `id` and advances its revision.
pub async fn save_automation(
    conn: &mut PgConnection,
    data: &Value,
    tenant: &str,
    id: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE customers SET automation=$1,revision=revision+1 WHERE tenant=$2 AND id=$3",
        data,
        tenant,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores the automation data of customer `id` only at revision `rev`; the number of rows changed.
pub async fn save_automation_at(
    a: &State,
    tenant: &str,
    id: &str,
    data: &Value,
    rev: i64,
) -> Result<u64> {
    Ok(sqlx::query!("UPDATE customers SET automation=$1,revision=revision+1 WHERE tenant=$2 AND id=$3 AND revision=$4", data, tenant, id, rev).execute(&a.db).await?.rows_affected())
}

/// How many customers belong to sales channel `channel`.
pub async fn channel_count(conn: &mut PgConnection, tenant: &str, channel: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM customers WHERE tenant=$1 AND sales_channel_id=$2"#,
        tenant,
        channel
    )
    .fetch_one(conn)
    .await?)
}
