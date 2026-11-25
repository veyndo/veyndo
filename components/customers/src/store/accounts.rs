//! Writes a customer makes to the own account: registration, the contact profile and the password.
use crate::service::address_book::{self, Save};
use crate::store::customers::{Customer, customer};
use serde_json::json;
use veyndo_core::{Result, State, conflict, store::history_context};
use veyndo_pricing::customer::{Address, Contact};

/// The account row as the customer reads it.
pub async fn row(a: &State, t: &str, email: &str) -> Result<Customer> {
    let r = sqlx::query(
        "SELECT *,created_at::text AS created FROM customers WHERE tenant=$1 AND email=$2",
    )
    .bind(t)
    .bind(email)
    .fetch_one(&a.db)
    .await?;
    Ok(customer(&r))
}

/// The price group that decides which payment methods the customer may prefer.
pub async fn group(a: &State, t: &str, email: &str) -> Result<String> {
    Ok(
        sqlx::query_scalar("SELECT group_name FROM customers WHERE tenant=$1 AND email=$2")
            .bind(t)
            .bind(email)
            .fetch_one(&a.db)
            .await?,
    )
}

/// An address handed in at registration and the defaults it becomes.
pub struct Initial {
    pub address: Address,
    pub default_billing: bool,
    pub default_shipping: bool,
}

/// Creates the account with its first addresses; an existing email is a conflict, never an update.
#[allow(clippy::too_many_arguments)]
pub async fn register(
    a: &State,
    t: &str,
    email: &str,
    password_hash: String,
    contact: &Contact,
    initial: Vec<Initial>,
    locale: &str,
    channel: &str,
) -> Result<()> {
    let mut tx = a.db.begin().await?;
    history_context::customer_context(&mut tx, email).await?;
    let n=sqlx::query("INSERT INTO customers(tenant,email,password_hash,profile) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(t).bind(email).bind(password_hash).bind(json!(contact)).execute(&mut *tx).await?.rows_affected();
    if n != 1 {
        return Err(conflict("Account already exists"));
    }
    for entry in initial {
        address_book::save(
            &mut tx,
            t,
            email,
            Save {
                id: None,
                revision: None,
                default_billing: entry.default_billing,
                default_shipping: entry.default_shipping,
                announce: true,
            },
            &entry.address,
        )
        .await?;
    }
    sqlx::query(
        "UPDATE customers SET language_id=$1,sales_channel_id=$2,company=NULLIF($5,'') WHERE tenant=$3 AND email=$4",
    )
    .bind(locale)
    .bind(channel)
    .bind(t)
    .bind(email)
    .bind(&contact.company)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Replaces the contact profile; an address in it becomes the default billing and shipping address.
pub async fn save_profile(a: &State, t: &str, email: &str, p: &Contact) -> Result<()> {
    let mut tx = a.db.begin().await?;
    history_context::customer_context(&mut tx, email).await?;
    sqlx::query("UPDATE customers SET profile=$1,company=NULLIF($2,''),revision=revision+1 WHERE tenant=$3 AND email=$4").bind(json!(p)).bind(&p.company).bind(t).bind(email).execute(&mut *tx).await?;
    if let Some(address) = &p.address {
        address_book::save(
            &mut tx,
            t,
            email,
            Save {
                id: None,
                revision: None,
                default_billing: true,
                default_shipping: true,
                announce: true,
            },
            address,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn password_hash(a: &State, t: &str, email: &str) -> Result<String> {
    Ok(
        sqlx::query_scalar("SELECT password_hash FROM customers WHERE tenant=$1 AND email=$2")
            .bind(t)
            .bind(email)
            .fetch_one(&a.db)
            .await?,
    )
}

/// Swaps the password hash only while the verified one is still stored, and ends every session of the account.
pub async fn replace_password(
    a: &State,
    t: &str,
    email: &str,
    current: String,
    new: String,
) -> Result<()> {
    let mut tx = a.db.begin().await?;
    history_context::customer_context(&mut tx, email).await?;
    let n = sqlx::query(
        "UPDATE customers SET password_hash=$1 WHERE tenant=$2 AND email=$3 AND password_hash=$4",
    )
    .bind(new)
    .bind(t)
    .bind(email)
    .bind(current)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n != 1 {
        return Err(conflict("Credential changed"));
    }
    sqlx::query("DELETE FROM customer_sessions WHERE tenant=$1 AND email=$2")
        .bind(t)
        .bind(email)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
