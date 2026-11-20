//! Personal merchant sessions: only hashed opaque tokens persist, and a second factor is honoured before one is issued.
use serde_json::{Value, json};
use sqlx::Row;
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
    sqlx::query("SELECT id FROM merchant_users WHERE id=$1 FOR UPDATE")
        .bind(user)
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
    let required: bool = sqlx::query_scalar(
        "SELECT coalesce((SELECT enabled FROM merchant_mfa WHERE user_id=$1),false)",
    )
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    if !verified_kernel::auth_factor_admissible(required, !matches!(proof, SessionProof::Primary)) {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Second factor required".into(),
        ));
    }
    let token = format!("{}{}", uid(), uid());
    sqlx::query("INSERT INTO user_sessions(digest,user_id,default_tenant) VALUES($1,$2,$3)")
        .bind(hash(&token))
        .bind(user)
        .bind(tenant)
        .execute(&mut **tx)
        .await?;
    let mut value = user_data_connection(tx, user).await?;
    value["token"] = json!(token);
    value["workspace"] = json!(tenant);
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
        sqlx::query("SELECT id,email,name,password_initialized,email_verified_at IS NOT NULL AS email_verified FROM merchant_users WHERE id=$1")
            .bind(user)
            .fetch_one(&mut *connection)
            .await?;
    let ms=sqlx::query("SELECT t.id,t.name,m.role FROM membership_grants m JOIN tenants t ON t.id=m.tenant WHERE m.user_id=$1 AND m.active ORDER BY t.name").bind(user).fetch_all(&mut *connection).await?;
    Ok(
        json!({"user":{"id":r.get::<String,_>("id"),"email":r.get::<String,_>("email"),"name":r.get::<String,_>("name"),"emailVerified":r.get::<bool,_>("email_verified")},"passwordSetupRequired":!r.get::<bool, _>("password_initialized"),"workspaces":ms.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"role":r.get::<String,_>("role"),"urls":links(&r.get::<String,_>("id"))})).collect::<Vec<_>>(),"expiresIn":43200}),
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
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_sessions WHERE user_id=$1 AND digest=$2 AND expires_at>now())").bind(user).bind(hash(token)).fetch_one(&mut **tx).await?)
}
pub async fn challenge_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    t: &str,
) -> Result<Option<Value>> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT coalesce((SELECT enabled FROM merchant_mfa WHERE user_id=$1),false)",
    )
    .bind(user)
    .fetch_one(&mut **tx)
    .await?;
    if !enabled {
        return Ok(None);
    }
    sqlx::query("DELETE FROM merchant_mfa_challenges WHERE user_id=$1 AND expires_at<=now()")
        .bind(user)
        .execute(&mut **tx)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM merchant_mfa_challenges WHERE user_id=$1")
            .bind(user)
            .fetch_one(&mut **tx)
            .await?;
    if count >= 12 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pending sign-ins. Wait five minutes.".into(),
        ));
    }
    let token = format!("{}{}", uid(), uid());
    sqlx::query("INSERT INTO merchant_mfa_challenges(digest,user_id,tenant) VALUES($1,$2,$3)")
        .bind(hash(&token))
        .bind(user)
        .bind(t)
        .execute(&mut **tx)
        .await?;
    Ok(Some(
        json!({"mfaRequired":true,"challengeToken":token,"expiresIn":300}),
    ))
}
