//! Customer rows as the cashier sees them: a bounded search and the account behind a selection.
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use veyndo_core::{Error, Result, State, StatusCode};

pub async fn search(a: &State, t: &str, query: &str, card: Option<&str>) -> Result<Vec<Value>> {
    let rows=sqlx::query("SELECT id,customer_number,email,profile->>'name' AS name,group_name FROM customers WHERE tenant=$1 AND active AND (($3::text IS NOT NULL AND customer_number=$3) OR ($3 IS NULL AND ($2='' OR strpos(lower(email||' '||customer_number||' '||coalesce(profile->>'name','')),lower($2))>0))) ORDER BY email LIMIT 20")
        .bind(t).bind(query).bind(card).fetch_all(&a.db).await?;
    Ok(rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"customerNumber":r.get::<String,_>("customer_number"),"email":r.get::<String,_>("email"),"name":r.get::<Option<String>,_>("name"),"customerGroup":r.get::<String,_>("group_name")})).collect())
}

/// The account a cashier attaches to a sale, read under a shared lock.
pub struct Account {
    pub email: String,
    pub group: String,
    pub company: Option<String>,
    pub default_billing_address_id: Option<String>,
}
pub async fn account(conn: &mut PgConnection, t: &str, id: &str) -> Result<Account> {
    let row=sqlx::query("SELECT id,email,group_name,company,default_billing_address_id FROM customers WHERE tenant=$1 AND id=$2 AND active FOR SHARE").bind(t).bind(id).fetch_optional(conn).await?.ok_or(Error(StatusCode::NOT_FOUND,"Customer unavailable".into()))?;
    Ok(Account {
        email: row.get("email"),
        group: row.get("group_name"),
        company: row.get("company"),
        default_billing_address_id: row.get("default_billing_address_id"),
    })
}
