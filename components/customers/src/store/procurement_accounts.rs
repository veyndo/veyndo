//! Customer accounts as company procurement reads them: the account behind a live customer session, existence and the contact fields of unit members. The B2B component reaches them through its records port. Columns checked at compile time.
use serde_json::Value;
use sqlx::PgConnection;
use veyndo_core::{Result, State};

/// The active customer behind the live session `digest`, share-locking the session and the account.
pub async fn session_customer(
    conn: &mut PgConnection,
    tenant: &str,
    digest: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!("SELECT c.id FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active FOR SHARE OF c,s", tenant, digest)
        .fetch_optional(conn)
        .await?)
}

/// Whether customer `id` exists.
pub async fn exists(conn: &mut PgConnection, tenant: &str, id: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM customers WHERE tenant=$1 AND id=$2) AS "v!""#,
        tenant,
        id
    )
    .fetch_one(conn)
    .await?)
}

/// The contact fields a company screen shows of a member.
pub struct Contact {
    pub id: String,
    pub email: String,
    pub profile: Value,
}

/// The contact fields of the customers among `ids`, in no particular order.
pub async fn contacts(a: &State, tenant: &str, ids: &[String]) -> Result<Vec<Contact>> {
    Ok(sqlx::query_as!(
        Contact,
        "SELECT id,email,profile FROM customers WHERE tenant=$1 AND id=ANY($2)",
        tenant,
        ids
    )
    .fetch_all(&a.db)
    .await?)
}
