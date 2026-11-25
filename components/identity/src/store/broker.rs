//! Trusted identity broker: assertions (signature binds route, audience, expiry and a one-use nonce) and the account recovery, linking and workspace reads they unlock.
use crate::domain::clock::now;
use crate::domain::credentials::email;
use crate::store::sessions::trusted_session_tx;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use veyndo_core::document::{lenient, view};
use veyndo_core::{Error, Result, State, StatusCode, bad, hash};

/// The JSON fields `assertion` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AssertionInput {
    #[serde(deserialize_with = "lenient")]
    payload: Option<String>,
    #[serde(deserialize_with = "lenient")]
    signature: Option<String>,
}

/// The signed claims of a broker assertion, typed as their readers use them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Claims {
    #[serde(deserialize_with = "lenient")]
    pub iss: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub sub: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub aud: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub verified: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub exp: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub iat: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub nonce: Option<String>,
    /// The broker's personal session, which proves a second factor already passed.
    #[serde(deserialize_with = "lenient")]
    pub session_token: Option<String>,
    /// The credential action (`set-password`, `verify-password`).
    #[serde(deserialize_with = "lenient")]
    pub action: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub workspace_id: Option<String>,
    /// The credential revision a recovery link was issued for.
    #[serde(deserialize_with = "lenient")]
    pub credential_revision: Option<i64>,
}

impl Claims {
    /// The verified (issuer, subject) pair; `assertion` admits only claims that carry both.
    pub fn subject(&self) -> (&str, &str) {
        (
            self.iss.as_deref().unwrap_or_default(),
            self.sub.as_deref().unwrap_or_default(),
        )
    }
}

/// Verifies a broker assertion for `route` and consumes its nonce; returns the signed claims.
pub async fn assertion(a: &State, route: &str, v: &Value) -> Result<Value> {
    let input: AssertionInput = view(v);
    let key = std::env::var("IDENTITY_BROKER_KEY")
        .ok()
        .filter(|k| k.len() >= 64)
        .ok_or(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Identity broker is not configured".into(),
        ))?;
    let issuer = std::env::var("IDENTITY_BROKER_ISSUER")
        .map_err(|_| bad("Identity issuer is not configured"))?;
    let limit = if route == "/api/identity/inference" {
        8_100_000
    } else {
        32000
    };
    let encoded = input
        .payload
        .as_deref()
        .filter(|s| s.len() <= limit)
        .ok_or(bad("Invalid identity assertion"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(input.signature.as_deref().unwrap_or(""))
        .map_err(|_| bad("Invalid assertion signature"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
        .map_err(|_| bad("Invalid broker configuration"))?;
    mac.update(format!("{route}\n{encoded}").as_bytes());
    mac.verify_slice(&signature).map_err(|_| {
        Error(
            StatusCode::UNAUTHORIZED,
            "Invalid identity assertion".into(),
        )
    })?;
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| bad("Invalid assertion encoding"))?,
    )
    .map_err(|_| bad("Invalid identity payload"))?;
    let now = now();
    let typed: Claims = view(&claims);
    if typed.iss.as_deref() != Some(issuer.as_str())
        || typed.aud.as_deref() != Some("veyndo-identity")
        || typed.verified != Some(true)
        || !typed.exp.is_some_and(|e| e > now && e <= now + 60)
        || !typed.iat.is_some_and(|i| i <= now + 5 && i >= now - 60)
        || !typed
            .nonce
            .as_deref()
            .is_some_and(|n| n.len() == 64 && n.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Expired or invalid identity assertion".into(),
        ));
    }
    email(&claims)?;
    if route == "/api/identity/inference" {
        let row=sqlx::query!(r#"SELECT u.id,coalesce(m.enabled,false) AS "required!" FROM merchant_users u LEFT JOIN merchant_mfa m ON m.user_id=u.id WHERE u.email=$1"#, email(&claims)?).fetch_optional(&a.db).await?;
        if let Some(row) = row
            && row.required
        {
            let mut tx = a.db.begin().await?;
            if !trusted_session_tx(&mut tx, &row.id, typed.session_token.as_deref()).await? {
                return Err(Error(
                    StatusCode::UNAUTHORIZED,
                    "Second factor required".into(),
                ));
            }
        }
    }
    if !typed
        .sub
        .as_deref()
        .is_some_and(|s| !s.is_empty() && s.len() <= 200)
    {
        return Err(bad("Identity subject required"));
    }
    sqlx::query!("DELETE FROM identity_nonces WHERE expires_at<now()")
        .execute(&a.db)
        .await?;
    let used=sqlx::query!("INSERT INTO identity_nonces(digest,expires_at) VALUES($1,now()+interval '2 minutes') ON CONFLICT DO NOTHING", hash(&format!("{}:{}",issuer,typed.nonce.as_deref().unwrap_or_default()))).execute(&a.db).await?.rows_affected();
    if used == 0 {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Identity assertion already consumed".into(),
        ));
    }
    Ok(claims)
}

/// The account a broker-verified identity may recover in a shop, locked for the credential change.
pub struct Recoverable {
    pub id: String,
    pub password_hash: String,
    pub password_initialized: bool,
    pub credential_revision: i64,
}

/// The account linked to the broker subject with this email and an active membership in the shop, locked.
pub async fn recoverable(
    tx: &mut sqlx::PgConnection,
    (issuer, subject): (&str, &str),
    email: &str,
    shop: &str,
) -> Result<Option<Recoverable>> {
    Ok(sqlx::query_as!(
        Recoverable,
        "SELECT u.id,u.password_hash,u.password_initialized,u.credential_revision FROM merchant_users u JOIN merchant_identities i ON i.user_id=u.id JOIN memberships m ON m.user_id=u.id WHERE i.issuer=$1 AND i.subject=$2 AND u.email=$3 AND m.tenant=$4 AND m.active FOR UPDATE OF u",
        issuer,
        subject,
        email,
        shop
    )
    .fetch_optional(tx)
    .await?)
}

/// Stores a new password hash, marks the password set and advances the credential revision.
pub async fn set_password(
    tx: &mut sqlx::PgConnection,
    user: &str,
    password_hash: String,
) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_users SET password_hash=$1,password_initialized=true,credential_revision=credential_revision+1 WHERE id=$2",
        password_hash,
        user
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// The account of an email and its credential revision, locked.
pub struct Account {
    pub id: String,
    pub credential_revision: i64,
}

/// Locks the account behind an email.
pub async fn lock_by_email(tx: &mut sqlx::PgConnection, email: &str) -> Result<Option<Account>> {
    Ok(sqlx::query_as!(
        Account,
        "SELECT id,credential_revision FROM merchant_users WHERE email=$1 FOR UPDATE",
        email
    )
    .fetch_optional(tx)
    .await?)
}

/// Whether the broker subject is linked to exactly this user.
pub async fn linked(
    tx: &mut sqlx::PgConnection,
    (issuer, subject): (&str, &str),
    user: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM merchant_identities WHERE issuer=$1 AND subject=$2 AND user_id=$3) AS "linked!""#,
        issuer,
        subject,
        user
    )
    .fetch_one(tx)
    .await?)
}

/// Records that the broker verified the account's email now.
pub async fn mark_verified(tx: &mut sqlx::PgConnection, user: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_users SET email_verified_at=now() WHERE id=$1",
        user
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Records the first broker verification of the account's email; a later one keeps the first time.
pub async fn mark_verified_once(a: &State, user: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE merchant_users SET email_verified_at=COALESCE(email_verified_at,now()) WHERE id=$1",
        user
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// An active shop the user is an active member of.
pub struct Workspace {
    pub id: String,
    pub name: String,
}

/// The user's active shops by name (at most 100).
pub async fn workspaces(tx: &mut sqlx::PgConnection, user: &str) -> Result<Vec<Workspace>> {
    Ok(sqlx::query_as!(
        Workspace,
        "SELECT t.id,t.name FROM membership_grants m JOIN tenants t ON t.id=m.tenant WHERE m.user_id=$1 AND m.active AND t.status='active' ORDER BY t.name LIMIT 100",
        user
    )
    .fetch_all(tx)
    .await?)
}

/// The account behind an email and its first active shop.
pub struct FirstWorkspace {
    pub id: String,
    pub tenant: String,
}

/// The account of the email with its first active membership in an active shop.
pub async fn first_workspace(a: &State, email: &str) -> Result<Option<FirstWorkspace>> {
    Ok(sqlx::query_as!(
        FirstWorkspace,
        r#"SELECT u.id,m.tenant AS "tenant!" FROM merchant_users u JOIN membership_grants m ON m.user_id=u.id AND m.active JOIN tenants t ON t.id=m.tenant AND t.status='active' WHERE u.email=$1 ORDER BY m.tenant LIMIT 1"#,
        email
    )
    .fetch_optional(&a.db)
    .await?)
}
