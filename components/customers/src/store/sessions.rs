//! Customer sessions, stored by the digest of their token. Columns checked at compile time.
use veyndo_core::{Result, State};

/// Opens a session for the customer under the digest of its token.
pub async fn insert(a: &State, digest: &str, t: &str, email: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO customer_sessions(digest,tenant,email) VALUES($1,$2,$3)",
        digest,
        t,
        email
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// The email of the active customer an unexpired session belongs to.
pub async fn email(a: &State, t: &str, digest: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT s.email FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active",
        t,
        digest
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Whether an unexpired session of an active customer has this digest.
pub async fn valid(a: &State, t: &str, digest: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active) AS "valid!""#,
        t,
        digest
    )
    .fetch_one(&a.db)
    .await?)
}

/// Ends one session.
pub async fn delete(a: &State, t: &str, digest: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM customer_sessions WHERE tenant=$1 AND digest=$2",
        t,
        digest
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
