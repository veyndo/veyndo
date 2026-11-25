//! Buyer sign-in records: the active account with its credential and defaults and the login times; the cart goes through the purchases port. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The parts of an active account a sign-in reads.
pub struct Account {
    pub id: String,
    pub password_hash: String,
    pub profile: Value,
    pub group_name: String,
    pub company: Option<String>,
    pub default_billing_address_id: Option<String>,
    pub default_shipping_address_id: Option<String>,
}

/// The active account of an email.
pub async fn active_account(a: &State, t: &str, email: &str) -> Result<Option<Account>> {
    Ok(sqlx::query_as!(
        Account,
        "SELECT id,password_hash,profile,group_name,company,default_billing_address_id,default_shipping_address_id FROM customers WHERE tenant=$1 AND email=$2 AND active",
        t,
        email
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Records the first and the latest login of the account.
pub async fn record_login(conn: &mut PgConnection, t: &str, email: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE customers SET first_login=coalesce(first_login,now()),last_login=now() WHERE tenant=$1 AND email=$2",
        t,
        email
    )
    .execute(conn)
    .await?;
    Ok(())
}
