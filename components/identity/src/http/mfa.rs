//! Personal MFA enrollment and one-use sign-in challenges share the native session and encryption owners.
use crate::domain::clock::now;
use crate::domain::credentials::{password, verify_password};
use crate::domain::totp;
use crate::store::sessions::{self, SessionProof, issue_verified_tx};
use crate::store::{merchant_accounts, mfa};
use crate::types::events::MfaChanged;
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_cognition::inference::settings;
use veyndo_core::document::{lenient, view};
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, hash, uid};

/// The live workspace behind a sandbox; account security is managed only where it is absent.
async fn parent(a: &veyndo_core::State, t: &str) -> Result<Option<String>> {
    mfa::live_shop(a, t).await
}
pub async fn personal<'a>(a: &veyndo_core::State, h: &'a RequestContext) -> Result<&'a str> {
    let user = header(h, "x-rac-user")
        .filter(|u| *u != "bootstrap" && h.principal.app.is_none())
        .ok_or(bad("Personal account required"))?;
    let token = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(bad("Personal session required"))?;
    let personal = veyndo_runtime::tenant_scope::scoped(
        veyndo_runtime::tenant_scope::Scope::System,
        sessions::live(a, &hash(token), user),
    )
    .await?;
    if !personal {
        return Err(bad("Personal session required"));
    }
    Ok(user)
}
fn unavailable() -> Error {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Account encryption is unavailable".into(),
    )
}
fn invalid() -> Error {
    Error(
        StatusCode::UNAUTHORIZED,
        "Invalid or expired authentication code".into(),
    )
}
pub async fn status(State(a): State<veyndo_core::State>, h: RequestContext) -> Result<Json<Value>> {
    let row = mfa::status(&a, personal(&a, &h).await?).await?;
    Ok(Json(row.map(|r|json!({"enabled":r.enabled,"revision":r.revision,"recoveryCodesRemaining":r.remaining})).unwrap_or(json!({"enabled":false,"revision":0,"recoveryCodesRemaining":0}))))
}
async fn password_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    v: &Value,
) -> Result<()> {
    let saved = merchant_accounts::password_hash_for_update(tx, user).await?;
    if !verify_password(password(v)?, saved).await? {
        return Err(invalid());
    }
    Ok(())
}
fn live(h: &RequestContext) -> Result<()> {
    if h.principal.app.is_some() || h.principal.user.as_deref() == Some("bootstrap") {
        return Err(bad("Personal account required"));
    }
    Ok(())
}
pub async fn enroll(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    live(&h)?;
    let user = personal(&a, &h).await?;
    if parent(&a, &tenant(&h)?).await?.is_some() {
        return Err(bad(
            "Account security is available only in a live workspace",
        ));
    }
    let secret = totp::secret().map_err(|_| unavailable())?;
    let cipher = settings::seal(&format!("mfa:{user}"), &secret).map_err(|_| unavailable())?;
    let mut tx = a.db.begin().await?;
    password_tx(&mut tx, user, &v).await?;
    let revision = mfa::begin_enrolment(&mut tx, user, &cipher)
        .await?
        .ok_or(conflict("MFA is already enabled"))?;
    let email = merchant_accounts::email(&mut tx, user).await?;
    tx.commit().await?;
    let mut url = reqwest::Url::parse("otpauth://totp/Veyndo").unwrap();
    url.set_path(&format!("Veyndo:{email}"));
    url.query_pairs_mut()
        .append_pair("secret", &secret)
        .append_pair("issuer", "Veyndo")
        .append_pair("algorithm", "SHA1")
        .append_pair("digits", "6")
        .append_pair("period", "30");
    Ok(Json(
        json!({"secret":secret,"uri":url.to_string(),"revision":revision,"expiresIn":600}),
    ))
}
/// One account security step: the expected revision, a one-time or recovery code, or a sign-in challenge.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MfaStep {
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    code: Option<String>,
    #[serde(deserialize_with = "lenient")]
    challenge_token: Option<String>,
}

impl MfaStep {
    fn code(&self) -> &str {
        self.code.as_deref().unwrap_or("")
    }
}

pub async fn confirm(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let user = personal(&a, &h).await?;
    if parent(&a, &tenant(&h)?).await?.is_some() {
        return Err(bad(
            "Account security is available only in a live workspace",
        ));
    }
    let mut tx = a.db.begin().await?;
    password_tx(&mut tx, user, &v).await?;
    let row = mfa::lock_pending(&mut tx, user).await?.ok_or(invalid())?;
    let input: MfaStep = view(&v);
    if input.revision != Some(row.revision) {
        return Err(conflict("Security settings changed; reload"));
    }
    let cipher = row.pending_ciphertext;
    let secret = settings::open(&format!("mfa:{user}"), &cipher).map_err(|_| unavailable())?;
    let step = totp::matched(&secret, input.code(), now(), -1).ok_or(invalid())?;
    let codes: Vec<String> = (0..8).map(|_| uid()).collect();
    let digests: Vec<String> = codes.iter().map(|c| hash(c)).collect();
    mfa::enable(&mut tx, user, &cipher, &json!(digests), step).await?;
    sessions::revoke_all(&mut tx, user).await?;
    let enabled = MfaChanged {
        user_id: user,
        action: "mfa.enabled",
        revision: row.revision + 1,
    };
    veyndo_core::events::emit(&mut *tx, &tenant(&h)?, &enabled).await?;
    let session =
        issue_verified_tx(&mut tx, user, &tenant(&h)?, SessionProof::SecondFactor).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"enabled":true,"recoveryCodes":codes,"session":session}),
    ))
}
async fn consume_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    code: &str,
) -> Result<()> {
    let row = mfa::lock_enabled(tx, user).await?.ok_or(invalid())?;
    let mut codes: Vec<String> =
        serde_json::from_value(row.recovery_digests).map_err(|_| invalid())?;
    if code.len() == 32
        && let Some(i) = codes.iter().position(|digest| digest == &hash(code))
    {
        codes.remove(i);
        mfa::set_recovery(tx, user, &json!(codes)).await?;
        return Ok(());
    }
    let secret =
        settings::open(&format!("mfa:{user}"), &row.ciphertext).map_err(|_| unavailable())?;
    let step = totp::matched(&secret, code, now(), row.last_step).ok_or(invalid())?;
    mfa::set_last_step(tx, user, step).await?;
    Ok(())
}
pub async fn disable(
    State(a): State<veyndo_core::State>,
    h: RequestContext,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let user = personal(&a, &h).await?;
    if parent(&a, &tenant(&h)?).await?.is_some() {
        return Err(bad(
            "Account security is available only in a live workspace",
        ));
    }
    let mut tx = a.db.begin().await?;
    password_tx(&mut tx, user, &v).await?;
    let input: MfaStep = view(&v);
    consume_code(&mut tx, user, input.code()).await?;
    let changed = mfa::disable(
        &mut tx,
        user,
        input.revision.ok_or(bad("Revision required"))?,
    )
    .await?;
    if changed != 1 {
        return Err(conflict("Security settings changed; reload"));
    }
    sessions::revoke_all(&mut tx, user).await?;
    let disabled = MfaChanged {
        user_id: user,
        action: "mfa.disabled",
        revision: input.revision.unwrap_or_default() + 1,
    };
    veyndo_core::events::emit(&mut *tx, &tenant(&h)?, &disabled).await?;
    tx.commit().await?;
    Ok(Json(json!({"enabled":false,"signInRequired":true})))
}
pub async fn verify(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let input: MfaStep = view(&v);
    let token = input
        .challenge_token
        .as_deref()
        .filter(|s| s.len() == 64)
        .ok_or(invalid())?;
    let row = mfa::attempt_challenge(&a, &hash(token))
        .await?
        .ok_or(invalid())?;
    let user = row.user_id;
    let t = row.tenant;
    let mut tx = a.db.begin().await?;
    merchant_accounts::lock(&mut tx, &user).await?;
    let valid = mfa::challenge_valid(&mut tx, &hash(token), &user, &t).await?;
    if !valid {
        return Err(invalid());
    }
    consume_code(&mut tx, &user, input.code()).await?;
    mfa::delete_challenge(&mut tx, &hash(token)).await?;
    let session = issue_verified_tx(&mut tx, &user, &t, SessionProof::SecondFactor).await?;
    tx.commit().await?;
    Ok(Json(session))
}
