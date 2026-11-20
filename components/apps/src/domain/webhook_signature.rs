//! Incoming app webhooks carry an HMAC-SHA-256 over the exact canonical message.
use hmac::{Hmac, Mac};
use sha2::Sha256;
use veyndo_core::{Error, Result, StatusCode};

/// Requires a 64-character hex signature of the canonical message under a secret of at least 32 bytes.
pub fn verify(secret: &str, canonical: &str, signature: &str) -> Result<()> {
    let denied = || {
        Error(
            StatusCode::UNAUTHORIZED,
            "Invalid app webhook signature".into(),
        )
    };
    if secret.len() < 32 || signature.len() != 64 || !signature.is_ascii() {
        return Err(denied());
    }
    let bytes = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&signature[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| denied())?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| denied())?;
    mac.update(canonical.as_bytes());
    mac.verify_slice(&bytes).map_err(|_| denied())
}
#[cfg(test)]
mod tests {
    use super::verify;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    #[test]
    fn signature_binds_the_exact_message() {
        let secret = "synthetic-test-secret-at-least-32-bytes";
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(b"fixture");
        let sig = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(verify(secret, "fixture", &sig).is_ok());
        assert!(verify(secret, "different shop", &sig).is_err());
        assert!(verify(secret, "fixture", &"é".repeat(32)).is_err());
    }
}
