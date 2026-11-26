//! Digest-bound per-shop app secrets on platform authenticated encryption; platform keys never leave this owner and there is no plaintext read path.
use crate::domain::approval;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_storage::secrets as shared;
use veyndo_core::{Error, Result, State, StatusCode, bad, conflict};

/// Platform authenticated encryption for app secrets.
pub struct Crypto;
impl shared::Crypto for Crypto {
    type Error = Error;
    fn seal(&self, binding: &str, plaintext: &str) -> Result<String> {
        veyndo_cognition::inference::settings::seal(binding, plaintext).map_err(|_| {
            Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Platform secret encryption is unavailable".into(),
            )
        })
    }
    fn open(&self, binding: &str, ciphertext: &str) -> Result<String> {
        veyndo_cognition::inference::settings::open(binding, ciphertext).map_err(|_| {
            Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "App secret cannot be decrypted; check the platform key".into(),
            )
        })
    }
}
/// The native error of a shared secret failure.
pub fn error(value: shared::SecretError<Error>) -> Error {
    match value {
        shared::SecretError::Database(e) => e.into(),
        shared::SecretError::Invalid(s) => bad(s),
        shared::SecretError::Conflict(s) => conflict(s),
        shared::SecretError::Crypto(e) => e,
    }
}
/// Shared encrypted persistence for transport credentials and declared app settings.
pub use veyndo_app_storage::secrets::Write as SecretWrite;
/// Rotates one secret of an app under its current package digest.
pub async fn store(a: &State, t: &str, id: &str, write: SecretWrite<'_>) -> Result<i64> {
    let mut tx = a.db.begin().await?;
    let revision = veyndo_app_storage::secrets::rotate(
        &mut tx,
        veyndo_app_storage::Scope { tenant: t, app: id },
        &write,
        &Crypto,
    )
    .await
    .map_err(error)?;
    tx.commit().await?;
    Ok(revision)
}
/// Existing operator fallbacks apply only when the scoped store has no configured value.
pub async fn resolve(a: &State, t: &str, m: &Manifest, kind: &str) -> Result<Option<String>> {
    let mut tx = a.db.begin().await?;
    let secret = veyndo_app_storage::secrets::resolve(
        &mut tx,
        veyndo_app_storage::Scope {
            tenant: t,
            app: &m.id,
        },
        kind,
        &approval::canonical_digest(m),
        &Crypto,
    )
    .await
    .map_err(error)?;
    tx.commit().await?;
    Ok(secret)
}
/// The original intake/receipt transaction retains the shared secret lock through verification.
pub async fn resolve_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
    kind: &str,
) -> Result<Option<String>> {
    veyndo_app_storage::secrets::resolve_locked(
        tx,
        veyndo_app_storage::Scope {
            tenant: t,
            app: &m.id,
        },
        kind,
        &approval::canonical_digest(m),
        &Crypto,
    )
    .await
    .map_err(error)
}
