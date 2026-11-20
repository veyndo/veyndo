//! Signed publisher namespaces and tenant-local dependency/version gates; the rules live in the apps component, this module supplies the operator's publisher keys.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
pub(super) use veyndo_app_platform::domain::distribution::NativeVerifier;
use veyndo_app_platform::domain::distribution::message;
pub(super) fn unsigned_namespace(m: &Manifest) -> Result<()> {
    veyndo_app_platform::domain::distribution::unsigned_namespace(
        m,
        &crate::runtime_config::get().publishers,
    )
}
pub(super) async fn dependencies(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    t: &str,
    m: &Manifest,
) -> Result<()> {
    veyndo_app_platform::store::dependencies::check(
        tx,
        t,
        m,
        &crate::runtime_config::get().publishers,
    )
    .await
}
/// Offline signing shares the server canonical representation. The 32-byte seed is supplied on stdin, never logged.
pub(crate) fn sign_package() {
    use ring::signature::KeyPair;
    use std::io::Read;
    let path = env::args()
        .nth(2)
        .expect("Usage: veyndo --sign-app MANIFEST.json < BASE64_SEED_FILE");
    let mut m: Manifest = serde_json::from_slice(&std::fs::read(path).expect("Read manifest"))
        .expect("Typed manifest");
    assert!(
        m.distribution.is_some(),
        "Add publisher/keyId/channel/distribution metadata first"
    );
    let mut secret = String::new();
    std::io::stdin()
        .take(100)
        .read_to_string(&mut secret)
        .expect("Read seed from stdin");
    let mut bytes = STANDARD
        .decode(secret.trim())
        .expect("Base64 32-byte Ed25519 seed");
    secret.clear();
    let pair =
        ring::signature::Ed25519KeyPair::from_seed_unchecked(&bytes).expect("32-byte Ed25519 seed");
    bytes.fill(0);
    m.distribution.as_mut().unwrap().signature = STANDARD.encode(pair.sign(&message(&m)).as_ref());
    // Public material is useful to register the publisher; private material is never emitted.
    println!(
        "{}",
        json!({"manifest":m,"publicKey":STANDARD.encode(pair.public_key().as_ref())})
    );
}
