//! The customer account facts checkout and order reads take from the customer directory, on the caller's connection. Columns checked at compile time.
use serde_json::Value;
use sqlx::{Executor, PgConnection, Postgres};
use veyndo_core::Result;

/// Locks the customer row for the rest of the transaction; `None` when the shop has no such customer.
pub async fn lock_id(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM customers WHERE tenant=$1 AND id=$2 FOR UPDATE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// The account binding a merchant selected for a cart.
pub struct Selected {
    pub email: String,
    pub group_name: String,
    pub company: Option<String>,
}

/// The active account's binding, share-locked so profile edits cannot race the purchase.
pub async fn share_selected(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<Selected>> {
    Ok(sqlx::query_as!(
        Selected,
        "SELECT email,group_name,company FROM customers WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// The account fields an order snapshot copies.
pub struct Snapshot {
    pub id: String,
    pub customer_number: String,
    pub email: String,
    pub profile: Value,
    pub company: Option<String>,
}

/// The active account's snapshot fields, share-locked until the purchase ends.
pub async fn share_snapshot(
    conn: &mut PgConnection,
    tenant: &str,
    id: &str,
) -> Result<Option<Snapshot>> {
    Ok(sqlx::query_as!(
        Snapshot,
        "SELECT id,customer_number,email,profile,company FROM customers WHERE tenant=$1 AND id=$2 AND active FOR SHARE",
        tenant,
        id
    )
    .fetch_optional(conn)
    .await?)
}

/// The account defaults a new cart of a signed-in buyer starts from.
pub struct CartDefaults {
    pub id: String,
    pub group_name: String,
    pub company: Option<String>,
    pub default_billing_address_id: Option<String>,
    pub default_shipping_address_id: Option<String>,
    pub profile: Value,
}

/// The cart defaults of the account with this email; a missing account is a database error.
pub async fn cart_defaults<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    email: &str,
) -> Result<CartDefaults> {
    Ok(sqlx::query_as!(
        CartDefaults,
        "SELECT id,group_name,company,default_billing_address_id,default_shipping_address_id,profile FROM customers WHERE tenant=$1 AND email=$2",
        tenant,
        email
    )
    .fetch_one(db)
    .await?)
}

/// The non-personal account facts a separately scoped app callback may read.
pub struct Redacted {
    pub id: String,
    pub active: bool,
    pub group_name: String,
    pub created: Option<String>,
}

/// The redacted account of this email, if any.
pub async fn redacted<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    email: &str,
) -> Result<Option<Redacted>> {
    Ok(sqlx::query_as!(
        Redacted,
        "SELECT id,active,group_name,created_at::text AS created FROM customers WHERE tenant=$1 AND email=$2",
        tenant,
        email
    )
    .fetch_optional(db)
    .await?)
}

/// The stable account ID of the email in the shop, if it has an account.
pub async fn id_by_email<'e>(
    db: impl Executor<'e, Database = Postgres>,
    tenant: &str,
    email: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM customers WHERE tenant=$1 AND email=$2",
        tenant,
        email
    )
    .fetch_optional(db)
    .await?)
}

/// The active customer of a current session (by token digest) of the shop.
pub async fn session_customer(
    conn: &mut PgConnection,
    tenant: &str,
    digest: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT c.id FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active",
        tenant,
        digest
    )
    .fetch_optional(conn)
    .await?)
}
