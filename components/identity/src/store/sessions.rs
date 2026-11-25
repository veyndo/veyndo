//! Personal merchant sessions: only hashed opaque tokens persist, a second factor is honoured before one is issued, and sessions are listed and revoked here.
use serde_json::{Value, json};
use veyndo_core::document::merge;
use veyndo_core::host_names::links;
use veyndo_core::{Error, Result, State, StatusCode, hash, uid};
use veyndo_kernel::verified_kernel;

/// How the person proved who they are before the session is issued.
pub enum SessionProof {
    Primary,
    SecondFactor,
    PersonalHandoff,
}

pub async fn issue_session(a: &State, user: &str, tenant: &str) -> Result<Value> {
    issue_session_with_proof(a, user, tenant, None).await
}
pub async fn issue_session_with_proof(
    a: &State,
    user: &str,
    tenant: &str,
    token: Option<&str>,
) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    sqlx::query!("SELECT id FROM merchant_users WHERE id=$1 FOR UPDATE", user)
        .fetch_one(&mut *tx)
        .await?;
    let trusted = trusted_session_tx(&mut tx, user, token).await?;
    let value = if trusted {
        issue_verified_tx(&mut tx, user, tenant, SessionProof::PersonalHandoff).await?
    } else if let Some(challenge) = challenge_tx(&mut tx, user, tenant).await? {
        challenge
    } else {
        issue_verified_tx(&mut tx, user, tenant, SessionProof::Primary).await?
    };
    tx.commit().await?;
    Ok(value)
}
pub async fn issue_verified_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    tenant: &str,
    proof: SessionProof,
) -> Result<Value> {
    let required: bool = sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT enabled FROM merchant_mfa WHERE user_id=$1),false) AS "required!""#,
        user
    )
    .fetch_one(&mut **tx)
    .await?;
    if !verified_kernel::auth_factor_admissible(required, !matches!(proof, SessionProof::Primary)) {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Second factor required".into(),
        ));
    }
    let token = format!("{}{}", uid(), uid());
    sqlx::query!(
        "INSERT INTO user_sessions(digest,user_id,default_tenant) VALUES($1,$2,$3)",
        hash(&token),
        user,
        tenant
    )
    .execute(&mut **tx)
    .await?;
    let mut value = user_data_connection(tx, user).await?;
    merge(&mut value, json!({"token":token,"workspace":tenant}))?;
    Ok(value)
}
pub async fn user_data(a: &State, user: &str) -> Result<Value> {
    let mut tx = a.db.begin().await?;
    let value = user_data_connection(&mut tx, user).await?;
    tx.commit().await?;
    Ok(value)
}
async fn user_data_connection(connection: &mut sqlx::PgConnection, user: &str) -> Result<Value> {
    let r =
        sqlx::query!(r#"SELECT id,email,name,password_initialized,email_verified_at IS NOT NULL AS "email_verified!" FROM merchant_users WHERE id=$1"#, user)
            .fetch_one(&mut *connection)
            .await?;
    let ms=sqlx::query!("SELECT t.id,t.name,m.role FROM membership_grants m JOIN tenants t ON t.id=m.tenant WHERE m.user_id=$1 AND m.active ORDER BY t.name", user).fetch_all(&mut *connection).await?;
    Ok(
        json!({"user":{"id":r.id,"email":r.email,"name":r.name,"emailVerified":r.email_verified},"passwordSetupRequired":!r.password_initialized,"workspaces":ms.iter().map(|r|json!({"id":r.id,"name":r.name,"role":r.role,"urls":links(&r.id)})).collect::<Vec<_>>(),"expiresIn":43200}),
    )
}
pub async fn trusted_session_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    token: Option<&str>,
) -> Result<bool> {
    let Some(token) = token else {
        return Ok(false);
    };
    Ok(sqlx::query_scalar!(r#"SELECT EXISTS(SELECT 1 FROM user_sessions WHERE user_id=$1 AND digest=$2 AND expires_at>now()) AS "trusted!""#, user, hash(token)).fetch_one(&mut **tx).await?)
}
pub async fn challenge_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    t: &str,
) -> Result<Option<Value>> {
    let enabled: bool = sqlx::query_scalar!(
        r#"SELECT coalesce((SELECT enabled FROM merchant_mfa WHERE user_id=$1),false) AS "required!""#,
        user
    )
    .fetch_one(&mut **tx)
    .await?;
    if !enabled {
        return Ok(None);
    }
    sqlx::query!(
        "DELETE FROM merchant_mfa_challenges WHERE user_id=$1 AND expires_at<=now()",
        user
    )
    .execute(&mut **tx)
    .await?;
    let count: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM merchant_mfa_challenges WHERE user_id=$1"#,
        user
    )
    .fetch_one(&mut **tx)
    .await?;
    if count >= 12 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pending sign-ins. Wait five minutes.".into(),
        ));
    }
    let token = format!("{}{}", uid(), uid());
    sqlx::query!(
        "INSERT INTO merchant_mfa_challenges(digest,user_id,tenant) VALUES($1,$2,$3)",
        hash(&token),
        user,
        t
    )
    .execute(&mut **tx)
    .await?;
    Ok(Some(
        json!({"mfaRequired":true,"challengeToken":token,"expiresIn":300}),
    ))
}

/// One live personal session of a person, as their session list shows it.
pub struct ActiveSession {
    pub digest: String,
    pub created: String,
    pub expires: String,
}

/// The person's live sessions, newest first (at most 100).
pub async fn active_sessions(a: &State, user: &str) -> Result<Vec<ActiveSession>> {
    Ok(sqlx::query_as!(
        ActiveSession,
        r#"SELECT digest,created_at::text AS "created!",expires_at::text AS "expires!" FROM user_sessions WHERE user_id=$1 AND expires_at>now() ORDER BY created_at DESC LIMIT 100"#,
        user
    )
    .fetch_all(&a.db)
    .await?)
}

/// Ends one session of the person; a digest of someone else's session changes nothing.
pub async fn revoke(a: &State, user: &str, digest: &str) -> Result<()> {
    sqlx::query!(
        "DELETE FROM user_sessions WHERE user_id=$1 AND digest=$2",
        user,
        digest
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// Ends the session behind a token digest (sign-out).
pub async fn end(a: &State, digest: &str) -> Result<()> {
    sqlx::query!("DELETE FROM user_sessions WHERE digest=$1", digest)
        .execute(&a.db)
        .await?;
    Ok(())
}

/// Whether the digest names a live session of the user (no tenant scope; callers run it as system work).
pub async fn live(a: &State, digest: &str, user: &str) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM user_sessions WHERE digest=$1 AND user_id=$2 AND expires_at>now()) AS "live!""#,
        digest,
        user
    )
    .fetch_one(&a.db)
    .await?)
}

/// Deletes every session, handoff ticket and pending sign-in challenge of the user (after a credential change).
pub async fn revoke_all(tx: &mut sqlx::PgConnection, user: &str) -> Result<()> {
    sqlx::query!("DELETE FROM user_sessions WHERE user_id=$1", user)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM merchant_handoffs WHERE user_id=$1", user)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM merchant_mfa_challenges WHERE user_id=$1", user)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

/// Deletes the user's sessions and handoff tickets (password recovery keeps pending sign-in challenges).
pub async fn revoke_sessions_and_handoffs(tx: &mut sqlx::PgConnection, user: &str) -> Result<()> {
    sqlx::query!("DELETE FROM user_sessions WHERE user_id=$1", user)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM merchant_handoffs WHERE user_id=$1", user)
        .execute(&mut *tx)
        .await?;
    Ok(())
}
