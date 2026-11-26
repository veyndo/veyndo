//! Tenant-owned address persistence, optimistic revisions and atomic default assignment. Columns checked at compile time.
use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

#[derive(Serialize)]
pub struct Entry {
    pub id: String,
    pub revision: i64,
    pub address: Value,
}

/// The first hundred addresses of a customer in the order they were added.
pub async fn list(a: &State, t: &str, email: &str) -> Result<Vec<Entry>> {
    Ok(sqlx::query_as!(
        Entry,
        "SELECT id,data AS address,revision FROM customer_addresses WHERE tenant=$1 AND email=$2 ORDER BY created_at,id LIMIT 100",
        t,
        email
    )
    .fetch_all(&a.db)
    .await?)
}

pub struct Defaults {
    pub default_billing_address_id: Option<String>,
    pub default_shipping_address_id: Option<String>,
    pub revision: i64,
}

pub async fn defaults(a: &State, t: &str, email: &str) -> Result<Option<Defaults>> {
    Ok(sqlx::query_as!(
        Defaults,
        "SELECT default_billing_address_id,default_shipping_address_id,revision FROM customers WHERE tenant=$1 AND email=$2",
        t,
        email
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Serializes the changes to one customer's address book; `false` when the customer does not exist.
pub async fn lock_customer(conn: &mut PgConnection, t: &str, email: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM customers WHERE tenant=$1 AND email=$2 FOR UPDATE",
        t,
        email
    )
    .fetch_optional(conn)
    .await?
    .is_some())
}

/// Replaces an address at the expected revision; `false` when it changed or is gone.
pub async fn update(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
    revision: i64,
    address: Value,
) -> Result<bool> {
    Ok(sqlx::query!(
        "UPDATE customer_addresses SET data=$1,revision=revision+1,updated_at=now() WHERE tenant=$2 AND email=$3 AND id=$4 AND revision=$5",
        address,
        t,
        email,
        id,
        revision
    )
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

pub async fn count(conn: &mut PgConnection, t: &str, email: &str) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM customer_addresses WHERE tenant=$1 AND email=$2"#,
        t,
        email
    )
    .fetch_one(conn)
    .await?)
}

pub async fn insert(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
    address: Value,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO customer_addresses(tenant,email,id,data) VALUES($1,$2,$3,$4)",
        t,
        email,
        id,
        address
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Makes the address a default where asked, and wherever the customer has none yet.
pub async fn assign_defaults(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
    billing: bool,
    shipping: bool,
) -> Result<()> {
    sqlx::query!(
        "UPDATE customers SET default_billing_address_id=CASE WHEN $4 OR default_billing_address_id IS NULL THEN $1 ELSE default_billing_address_id END,default_shipping_address_id=CASE WHEN $5 OR default_shipping_address_id IS NULL THEN $1 ELSE default_shipping_address_id END,revision=revision+1 WHERE tenant=$2 AND email=$3",
        id,
        t,
        email,
        billing,
        shipping
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Deletes an address at the expected revision; `false` when it changed or is gone.
pub async fn delete(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
    revision: i64,
) -> Result<bool> {
    Ok(sqlx::query!(
        "DELETE FROM customer_addresses WHERE tenant=$1 AND email=$2 AND id=$3 AND revision=$4",
        t,
        email,
        id,
        revision
    )
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

pub async fn touch_customer(conn: &mut PgConnection, t: &str, email: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE customers SET revision=revision+1 WHERE tenant=$1 AND email=$2",
        t,
        email
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The stored address, share-locked so it cannot change before the transaction commits.
pub async fn get_shared(
    conn: &mut PgConnection,
    t: &str,
    email: &str,
    id: &str,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar!(
        "SELECT data FROM customer_addresses WHERE tenant=$1 AND email=$2 AND id=$3 FOR SHARE",
        t,
        email,
        id
    )
    .fetch_optional(conn)
    .await?)
}
