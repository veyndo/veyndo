//! Argon2 password operations run off the asynchronous request executor.
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use serde_json::Value;
use uuid::Uuid;
use veyndo_core::{Result, bad};
pub fn email(v: &Value) -> Result<String> {
    let s = v["email"].as_str().unwrap_or("").trim().to_lowercase();
    if s.len() > 254
        || s.len() < 3
        || s.split('@').count() != 2
        || s.starts_with('@')
        || s.ends_with('@')
        || s.chars().any(char::is_whitespace)
    {
        return Err(bad("Valid email required"));
    }
    Ok(s)
}
pub fn password(v: &Value) -> Result<String> {
    let p = v["password"].as_str().unwrap_or("");
    if !(12..=128).contains(&p.len()) {
        return Err(bad("Password must contain 12..128 characters"));
    }
    Ok(p.into())
}
pub fn name(v: &Value) -> Result<String> {
    let n = v["name"].as_str().unwrap_or("").trim();
    if n.is_empty() || n.len() > 100 {
        return Err(bad("Name must contain 1..100 characters"));
    }
    Ok(n.into())
}
pub async fn hash_password(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).unwrap();
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|p| p.to_string())
            .map_err(|_| bad("Password hashing failed"))
    })
    .await
    .map_err(|_| bad("Password worker failed"))?
}
pub async fn verify_password(password: String, saved: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&saved).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(|_| bad("Password worker failed"))
}
pub fn role_allowed(actor: &str, target: &str) -> bool {
    veyndo_app_model::declarations::identifier(target)
        && (actor == "owner" || actor == "admin" && target != "owner")
}
/// HMAC verification compares fixed-size authenticators in constant time, including different token lengths.
pub fn bootstrap_matches(candidate: &str, expected: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut saved = Hmac::<Sha256>::new_from_slice(b"veyndo-bootstrap-comparison").unwrap();
    saved.update(expected.as_bytes());
    let authenticator = saved.finalize().into_bytes();
    let mut supplied = Hmac::<Sha256>::new_from_slice(b"veyndo-bootstrap-comparison").unwrap();
    supplied.update(candidate.as_bytes());
    !expected.is_empty() && supplied.verify_slice(&authenticator).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{
        bootstrap_matches, email, hash_password, name, password, role_allowed, verify_password,
    };

    use serde_json::json;
    #[test]
    fn validation_and_role_boundaries() {
        assert!(email(&json!({"email":" x@example.test "})).is_ok());
        assert!(email(&json!({"email":"a@@b"})).is_err());
        assert!(password(&json!({"password":"short"})).is_err());
        assert!(role_allowed("owner", "owner"));
        assert!(!role_allowed("admin", "owner"));
        assert!(!role_allowed("editor", "viewer"));
    }
    #[tokio::test]
    async fn password_round_trip() {
        let saved = hash_password("a-test-password-123".into()).await.unwrap();
        assert!(
            verify_password("a-test-password-123".into(), saved.clone())
                .await
                .unwrap()
        );
        assert!(
            !verify_password("a-wrong-password-123".into(), saved)
                .await
                .unwrap()
        );
    }
    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(
            name(&json!({"name":"  Ada  "})).ok().as_deref(),
            Some("Ada")
        );
        assert!(name(&json!({"name":"   "})).is_err());
        assert!(name(&json!({})).is_err());
        assert!(name(&json!({"name":"x".repeat(101)})).is_err());
        assert!(name(&json!({"name":"x".repeat(100)})).is_ok());
    }
    #[test]
    fn the_bootstrap_secret_matches_only_itself_and_never_when_unset() {
        assert!(bootstrap_matches(
            "a-long-bootstrap-secret",
            "a-long-bootstrap-secret"
        ));
        assert!(!bootstrap_matches(
            "a-long-bootstrap-secreT",
            "a-long-bootstrap-secret"
        ));
        assert!(!bootstrap_matches("a-long", "a-long-bootstrap-secret"));
        assert!(!bootstrap_matches("", "a-long-bootstrap-secret"));
        assert!(!bootstrap_matches("", ""));
    }
}
