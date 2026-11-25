//! Customer rows as the cashier sees them: a bounded search and the account behind a selection.
use serde_json::{Value, json};
use sqlx::PgConnection;
use veyndo_core::{Error, Result, State, StatusCode};

pub async fn search(a: &State, t: &str, query: &str, card: Option<&str>) -> Result<Vec<Value>> {
    let rows=sqlx::query!("SELECT id,customer_number,email,profile->>'name' AS name,group_name FROM customers WHERE tenant=$1 AND active AND (($3::text IS NOT NULL AND customer_number=$3) OR ($3 IS NULL AND ($2='' OR strpos(lower(email||' '||customer_number||' '||coalesce(profile->>'name','')),lower($2))>0))) ORDER BY email LIMIT 20",
        t, query, card).fetch_all(&a.db).await?;
    Ok(rows.into_iter().map(|r|json!({"id":r.id,"customerNumber":r.customer_number,"email":r.email,"name":r.name,"customerGroup":r.group_name})).collect())
}

/// The account a cashier attaches to a sale, read under a shared lock.
pub struct Account {
    pub email: String,
    pub group: String,
    pub company: Option<String>,
    pub default_billing_address_id: Option<String>,
}
pub async fn account(conn: &mut PgConnection, t: &str, id: &str) -> Result<Account> {
    sqlx::query_as!(Account,r#"SELECT email,group_name AS "group",company,default_billing_address_id FROM customers WHERE tenant=$1 AND id=$2 AND active FOR SHARE"#,t,id).fetch_optional(conn).await?.ok_or(Error(StatusCode::NOT_FOUND,"Customer unavailable".into()))
}
