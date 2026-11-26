//! Independent customer sessions: a random token whose digest alone is stored.
use crate::store::sessions;
use veyndo_core::{Error, Result, State, StatusCode, hash, uid};

/// Opens a session and returns its token; the token is never stored.
pub async fn open(a: &State, t: &str, email: &str) -> Result<String> {
    let token = uid();
    sessions::insert(a, &hash(&token), t, email).await?;
    Ok(token)
}

/// The email of the signed-in customer; refuses expired sessions and deactivated accounts.
pub async fn identify(a: &State, t: &str, digest: &str) -> Result<String> {
    sessions::email(a, t, digest).await?.ok_or(Error(
        StatusCode::UNAUTHORIZED,
        "Customer session expired".into(),
    ))
}

pub async fn close(a: &State, t: &str, token: &str) -> Result<()> {
    sessions::delete(a, t, &hash(token)).await
}
