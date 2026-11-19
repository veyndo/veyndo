//! Customer sessions, stored by the digest of their token.
use veyndo_core::{Result, State};

pub async fn insert(a: &State, digest: &str, t: &str, email: &str) -> Result<()> {
    sqlx::query("INSERT INTO customer_sessions(digest,tenant,email) VALUES($1,$2,$3)")
        .bind(digest)
        .bind(t)
        .bind(email)
        .execute(&a.db)
        .await?;
    Ok(())
}

/// The email of the active customer an unexpired session belongs to.
pub async fn email(a: &State, t: &str, digest: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT s.email FROM customer_sessions s JOIN customers c ON c.tenant=s.tenant AND c.email=s.email WHERE s.tenant=$1 AND s.digest=$2 AND s.expires_at>now() AND c.active",
    )
    .bind(t)
    .bind(digest)
    .fetch_optional(&a.db)
    .await?)
}

pub async fn delete(a: &State, t: &str, digest: &str) -> Result<()> {
    sqlx::query("DELETE FROM customer_sessions WHERE tenant=$1 AND digest=$2")
        .bind(t)
        .bind(digest)
        .execute(&a.db)
        .await?;
    Ok(())
}
