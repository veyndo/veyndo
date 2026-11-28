//! Signed publisher namespaces, package contracts and tenant-local dependency/version gates; the rules live in the apps component, this module supplies offline signing.
use super::Manifest;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use std::env;
use veyndo_app_platform::domain::distribution::message;
use veyndo_app_platform::domain::package_contract::{self, Contract};
/// Offline signing shares the server canonical representation. The 32-byte seed is supplied on stdin, never logged.
pub(crate) fn sign_package() {
    use ring::signature::KeyPair;
    use std::io::Read;
    let path = env::args()
        .nth(2)
        .expect("Usage: veyndo --sign-app MANIFEST.json [CONTRACT.json] < BASE64_SEED_FILE");
    let contract: Option<Contract> = env::args().nth(3).map(|path| {
        serde_json::from_slice(&std::fs::read(path).expect("Read contract"))
            .expect("Typed contract")
    });
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
    // The contract signature binds the signed Manifest, so it is made last.
    let contract = contract.map(|mut c| {
        c.signature = None;
        c.signature = Some(STANDARD.encode(pair.sign(&package_contract::message(&m, &c)).as_ref()));
        c
    });
    // Public material is useful to register the publisher; private material is never emitted.
    println!(
        "{}",
        json!({"manifest":m,"contract":contract,"publicKey":STANDARD.encode(pair.public_key().as_ref())})
    );
}
