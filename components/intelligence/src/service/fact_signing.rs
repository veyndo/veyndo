//! Public Ed25519 attestations bind exact native facts and channel context for a short time; signatures do not guarantee source truth.
use axum::Json;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veyndo_core::{Error, Result, StatusCode, bad, hash};

fn key() -> Result<Ed25519KeyPair> {
    let secret = std::env::var("FACT_SIGNING_SEED")
        .or_else(|_| std::env::var("PLATFORM_SECRET_KEY"))
        .map_err(|_| {
            Error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Fact signing is not configured".into(),
            )
        })?;
    if secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad("Fact signing seed must be 32-byte hexadecimal"));
    }
    let bytes = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&secret[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let seed = Sha256::digest(
        [
            b"veyndo/public-facts/ed25519/v1".as_slice(),
            bytes.as_slice(),
        ]
        .concat(),
    );
    Ed25519KeyPair::from_seed_unchecked(&seed).map_err(|_| bad("Invalid fact signing key"))
}
/// Signs a payload and returns it with its base64 bytes, signature and key identifier.
pub fn attest(payload: Value) -> Result<Value> {
    let key = key()?;
    let bytes = serde_json::to_vec(&payload).map_err(|_| bad("Invalid fact payload"))?;
    let public = URL_SAFE_NO_PAD.encode(key.public_key().as_ref());
    Ok(
        json!({"payload":payload,"payloadBase64":URL_SAFE_NO_PAD.encode(&bytes),"signature":URL_SAFE_NO_PAD.encode(key.sign(&bytes).as_ref()),"algorithm":"Ed25519","keyId":hash(&public),"verificationKeys":"/.well-known/commerce-facts.json"}),
    )
}
/// Identifier of the public key agents verify against; absent when signing is not configured.
pub fn key_id() -> Option<String> {
    key()
        .ok()
        .map(|key| hash(&URL_SAFE_NO_PAD.encode(key.public_key().as_ref())))
}
/// Verifies bytes against this installation's own public key.
pub fn verify(bytes: &[u8], signature: &[u8]) -> Result<bool> {
    let key = key()?;
    Ok(ring::signature::UnparsedPublicKey::new(
        &ring::signature::ED25519,
        key.public_key().as_ref(),
    )
    .verify(bytes, signature)
    .is_ok())
}
/// The public verification keys of this installation.
pub async fn keys() -> Result<Json<Value>> {
    let public = URL_SAFE_NO_PAD.encode(key()?.public_key().as_ref());
    Ok(Json(
        json!({"keys":[{"kty":"OKP","crv":"Ed25519","x":public,"kid":hash(&public),"use":"sig","alg":"EdDSA"}],"schemaVersion":1}),
    ))
}
#[cfg(test)]
mod tests {
    #[test]
    fn signatures_bind_exact_bytes_and_reject_modified_evidence() {
        use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
        let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let data = b"{\"stock\":4,\"expiresAt\":300}";
        let sig = key.sign(data);
        let verifier = UnparsedPublicKey::new(&ED25519, key.public_key().as_ref());
        assert!(verifier.verify(data, sig.as_ref()).is_ok());
        assert!(
            verifier
                .verify(b"{\"stock\":5,\"expiresAt\":300}", sig.as_ref())
                .is_err()
        );
    }
}
