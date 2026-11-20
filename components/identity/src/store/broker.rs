//! Trusted identity broker assertions: the signature binds route, audience, expiry and a one-use nonce.
use crate::domain::clock::now;
use crate::domain::credentials::email;
use crate::store::sessions::trusted_session_tx;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use sqlx::Row;
use veyndo_core::{Error, Result, State, StatusCode, bad, hash};

/// Verifies a broker assertion for `route` and consumes its nonce; returns the signed claims.
pub async fn assertion(a: &State, route: &str, v: &Value) -> Result<Value> {
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
    let encoded = v["payload"]
        .as_str()
        .filter(|s| s.len() <= limit)
        .ok_or(bad("Invalid identity assertion"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(v["signature"].as_str().unwrap_or(""))
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
    if claims["iss"] != issuer
        || claims["aud"] != "veyndo-identity"
        || claims["verified"] != true
        || !claims["exp"]
            .as_i64()
            .is_some_and(|e| e > now && e <= now + 60)
        || !claims["iat"]
            .as_i64()
            .is_some_and(|i| i <= now + 5 && i >= now - 60)
        || !claims["nonce"]
            .as_str()
            .is_some_and(|n| n.len() == 64 && n.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Expired or invalid identity assertion".into(),
        ));
    }
    email(&claims)?;
    if route == "/api/identity/inference" {
        let row=sqlx::query("SELECT u.id,coalesce(m.enabled,false) AS required FROM merchant_users u LEFT JOIN merchant_mfa m ON m.user_id=u.id WHERE u.email=$1").bind(email(&claims)?).fetch_optional(&a.db).await?;
        if let Some(row) = row
            && row.get::<bool, _>("required")
        {
            let mut tx = a.db.begin().await?;
            if !trusted_session_tx(
                &mut tx,
                &row.get::<String, _>("id"),
                claims["sessionToken"].as_str(),
            )
            .await?
            {
                return Err(Error(
                    StatusCode::UNAUTHORIZED,
                    "Second factor required".into(),
                ));
            }
        }
    }
    if !claims["sub"]
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 200)
    {
        return Err(bad("Identity subject required"));
    }
    sqlx::query("DELETE FROM identity_nonces WHERE expires_at<now()")
        .execute(&a.db)
        .await?;
    let used=sqlx::query("INSERT INTO identity_nonces(digest,expires_at) VALUES($1,now()+interval '2 minutes') ON CONFLICT DO NOTHING")
        .bind(hash(&format!("{}:{}",issuer,claims["nonce"].as_str().unwrap()))).execute(&a.db).await?.rows_affected();
    if used == 0 {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Identity assertion already consumed".into(),
        ));
    }
    Ok(claims)
}
