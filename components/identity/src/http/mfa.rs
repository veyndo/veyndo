//! Personal MFA enrollment and one-use sign-in challenges share the native session and encryption owners.
use crate::domain::clock::now;
use crate::domain::credentials::{password, verify_password};
use crate::domain::totp;
use crate::store::sessions::{SessionProof, issue_verified_tx};
use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use sqlx::Row;
use veyndo_cognition::inference::settings;
use veyndo_core::shop_scope::{header, tenant};
use veyndo_core::{Error, RequestContext, Result, StatusCode, bad, conflict, hash, uid};

/// The live workspace behind a sandbox; account security is managed only where it is absent.
async fn parent(a: &veyndo_core::State, t: &str) -> Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT live_tenant FROM shop_environments WHERE tenant=$1")
            .bind(t)
            .fetch_optional(&a.db)
            .await?,
    )
}
pub async fn personal<'a>(a: &veyndo_core::State, h: &'a RequestContext) -> Result<&'a str> {
    let user = header(h, "x-rac-user")
        .filter(|u| *u != "bootstrap" && h.principal.app.is_none())
        .ok_or(bad("Personal account required"))?;
    let token = header(h, "authorization")
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(bad("Personal session required"))?;
    let personal:bool=veyndo_runtime::tenant_scope::scoped(veyndo_runtime::tenant_scope::Scope::System,
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_sessions WHERE digest=$1 AND user_id=$2 AND expires_at>now())").bind(hash(token)).bind(user).fetch_one(&a.db)).await?;
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
    let row=sqlx::query("SELECT enabled,revision,jsonb_array_length(recovery_digests) AS remaining FROM merchant_mfa WHERE user_id=$1").bind(personal(&a,&h).await?).fetch_optional(&a.db).await?;
    Ok(Json(row.map(|r|json!({"enabled":r.get::<bool,_>("enabled"),"revision":r.get::<i64,_>("revision"),"recoveryCodesRemaining":r.get::<i32,_>("remaining")})).unwrap_or(json!({"enabled":false,"revision":0,"recoveryCodesRemaining":0}))))
}
async fn password_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    v: &Value,
) -> Result<()> {
    let saved: String =
        sqlx::query_scalar("SELECT password_hash FROM merchant_users WHERE id=$1 FOR UPDATE")
            .bind(user)
            .fetch_one(&mut **tx)
            .await?;
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
    let row=sqlx::query("INSERT INTO merchant_mfa(user_id,pending_ciphertext,pending_expires_at) VALUES($1,$2,now()+interval '10 minutes') ON CONFLICT(user_id) DO UPDATE SET pending_ciphertext=EXCLUDED.pending_ciphertext,pending_expires_at=EXCLUDED.pending_expires_at WHERE NOT merchant_mfa.enabled RETURNING revision").bind(user).bind(cipher).fetch_optional(&mut *tx).await?.ok_or(conflict("MFA is already enabled"))?;
    let email: String = sqlx::query_scalar("SELECT email FROM merchant_users WHERE id=$1")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
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
        json!({"secret":secret,"uri":url.to_string(),"revision":row.get::<i64,_>("revision"),"expiresIn":600}),
    ))
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
    let row=sqlx::query("SELECT pending_ciphertext,revision FROM merchant_mfa WHERE user_id=$1 AND NOT enabled AND pending_expires_at>now() FOR UPDATE").bind(user).fetch_optional(&mut *tx).await?.ok_or(invalid())?;
    if v["revision"].as_i64() != Some(row.get("revision")) {
        return Err(conflict("Security settings changed; reload"));
    }
    let cipher: String = row.get("pending_ciphertext");
    let secret = settings::open(&format!("mfa:{user}"), &cipher).map_err(|_| unavailable())?;
    let step =
        totp::matched(&secret, v["code"].as_str().unwrap_or(""), now(), -1).ok_or(invalid())?;
    let codes: Vec<String> = (0..8).map(|_| uid()).collect();
    let digests: Vec<String> = codes.iter().map(|c| hash(c)).collect();
    sqlx::query("UPDATE merchant_mfa SET ciphertext=$2,enabled=true,pending_ciphertext=NULL,pending_expires_at=NULL,recovery_digests=$3,last_step=$4,revision=revision+1,updated_at=now() WHERE user_id=$1").bind(user).bind(cipher).bind(json!(digests)).bind(step).execute(&mut *tx).await?;
    revoke_tx(&mut tx, user).await?;
    veyndo_core::outbox::publish_on(
        &mut *tx,
        &tenant(&h)?,
        "account.security.updated",
        &json!({"userId":user,"action":"mfa.enabled","revision":row.get::<i64,_>("revision")+1}),
    )
    .await?;
    let session =
        issue_verified_tx(&mut tx, user, &tenant(&h)?, SessionProof::SecondFactor).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"enabled":true,"recoveryCodes":codes,"session":session}),
    ))
}
async fn revoke_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, user: &str) -> Result<()> {
    for sql in [
        "DELETE FROM user_sessions WHERE user_id=$1",
        "DELETE FROM merchant_handoffs WHERE user_id=$1",
        "DELETE FROM merchant_mfa_challenges WHERE user_id=$1",
    ] {
        sqlx::query(sql).bind(user).execute(&mut **tx).await?;
    }
    Ok(())
}
async fn consume_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &str,
    code: &str,
) -> Result<()> {
    let row=sqlx::query("SELECT ciphertext,last_step,recovery_digests FROM merchant_mfa WHERE user_id=$1 AND enabled FOR UPDATE").bind(user).fetch_optional(&mut **tx).await?.ok_or(invalid())?;
    let mut codes: Vec<String> =
        serde_json::from_value(row.get("recovery_digests")).map_err(|_| invalid())?;
    if code.len() == 32
        && let Some(i) = codes.iter().position(|digest| digest == &hash(code))
    {
        codes.remove(i);
        sqlx::query("UPDATE merchant_mfa SET recovery_digests=$2 WHERE user_id=$1")
            .bind(user)
            .bind(json!(codes))
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }
    let secret = settings::open(&format!("mfa:{user}"), &row.get::<String, _>("ciphertext"))
        .map_err(|_| unavailable())?;
    let step = totp::matched(&secret, code, now(), row.get("last_step")).ok_or(invalid())?;
    sqlx::query("UPDATE merchant_mfa SET last_step=$2 WHERE user_id=$1")
        .bind(user)
        .bind(step)
        .execute(&mut **tx)
        .await?;
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
    consume_code(&mut tx, user, v["code"].as_str().unwrap_or("")).await?;
    let changed=sqlx::query("UPDATE merchant_mfa SET enabled=false,ciphertext=NULL,recovery_digests='[]',pending_ciphertext=NULL,pending_expires_at=NULL,revision=revision+1,updated_at=now() WHERE user_id=$1 AND revision=$2").bind(user).bind(v["revision"].as_i64().ok_or(bad("Revision required"))?).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(conflict("Security settings changed; reload"));
    }
    revoke_tx(&mut tx, user).await?;
    veyndo_core::outbox::publish_on(&mut *tx, &tenant(&h)?, "account.security.updated", &json!({"userId":user,"action":"mfa.disabled","revision":v["revision"].as_i64().unwrap()+1})).await?;
    tx.commit().await?;
    Ok(Json(json!({"enabled":false,"signInRequired":true})))
}
pub async fn verify(
    State(a): State<veyndo_core::State>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let token = v["challengeToken"]
        .as_str()
        .filter(|s| s.len() == 64)
        .ok_or(invalid())?;
    let row=sqlx::query("UPDATE merchant_mfa_challenges SET attempts=attempts+1 WHERE digest=$1 AND expires_at>now() AND attempts<5 RETURNING user_id,tenant").bind(hash(token)).fetch_optional(&a.db).await?.ok_or(invalid())?;
    let user: String = row.get("user_id");
    let t: String = row.get("tenant");
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT id FROM merchant_users WHERE id=$1 FOR UPDATE")
        .bind(&user)
        .fetch_one(&mut *tx)
        .await?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM merchant_mfa_challenges WHERE digest=$1 AND expires_at>now()) AND EXISTS(SELECT 1 FROM membership_grants WHERE user_id=$2 AND tenant=$3 AND active)").bind(hash(token)).bind(&user).bind(&t).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(invalid());
    }
    consume_code(&mut tx, &user, v["code"].as_str().unwrap_or("")).await?;
    sqlx::query("DELETE FROM merchant_mfa_challenges WHERE digest=$1")
        .bind(hash(token))
        .execute(&mut *tx)
        .await?;
    let session = issue_verified_tx(&mut tx, &user, &t, SessionProof::SecondFactor).await?;
    tx.commit().await?;
    Ok(Json(session))
}
