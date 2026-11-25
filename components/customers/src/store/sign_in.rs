//! Buyer sign-in records: the active account with its credential and defaults, the open cart it binds and the login times. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};
use veyndo_shop::types::cart::{CartRow, StoredCart};

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

/// The cart of a context token under an update lock.
pub async fn lock_cart(conn: &mut PgConnection, t: &str, token: &str) -> Result<StoredCart> {
    sqlx::query_as!(
        CartRow,
        "SELECT id,tenant,token,data,revision,status FROM carts WHERE tenant=$1 AND token=$2 FOR UPDATE",
        t,
        token
    )
    .fetch_one(conn)
    .await?
    .hydrate()
}

/// Binds the cart document to the account and rotates its context token.
pub async fn bind_cart(conn: &mut PgConnection, id: &str, data: &Value, token: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE carts SET data=$1,token=$2,revision=revision+1 WHERE id=$3",
        data,
        token,
        id
    )
    .execute(conn)
    .await?;
    Ok(())
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
