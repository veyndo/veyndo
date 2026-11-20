//! Signed publisher namespaces and tenant-local dependency/version gates. Operator service access still requires its separate exact digest pin.
use crate::domain::model_contract;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use veyndo_app_model::distribution as shared;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, Result, StatusCode, bad};

/// The exact bytes a publisher signs for a package.
pub fn message(m: &Manifest) -> Vec<u8> {
    veyndo_app_model::manifest::signature_message(m).expect("Signed package")
}
/// Verifies a package signature against the publisher keys the operator registered.
pub struct NativeVerifier<'a>(pub &'a Value);
impl shared::SignatureVerifier for NativeVerifier<'_> {
    type Error = Error;
    fn verify_signature(&self, m: &Manifest) -> Result<()> {
        let d = m.distribution.as_ref().expect("Signed package");
        let public = self.0[&d.publisher][&d.key_id].as_str().ok_or(Error(
            StatusCode::FORBIDDEN,
            "Publisher key is not registered by the operator".into(),
        ))?;
        let key = STANDARD
            .decode(public)
            .map_err(|_| bad("Invalid registered publisher public key"))?;
        let signature = STANDARD
            .decode(&d.signature)
            .map_err(|_| bad("Invalid package signature encoding"))?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
            .verify(&message(m), &signature)
            .map_err(|_| {
                Error(
                    StatusCode::FORBIDDEN,
                    "Package signature does not match its content".into(),
                )
            })?;
        Ok(())
    }
}
/// Checks the distribution metadata and the signature of a package.
pub fn verify(m: &Manifest, keys: &Value) -> Result<()> {
    shared::verify(m, &NativeVerifier(keys)).map_err(|e| match e {
        shared::VerifyError::Model(e) => model_contract::error(e),
        shared::VerifyError::Host(e) => e,
    })
}
/// Refuses an unsigned package whose ID lies in the namespace of a registered publisher.
pub fn unsigned_namespace(m: &Manifest, publishers: &Value) -> Result<()> {
    // A local unsigned app cannot occupy a registered publisher's namespace.
    if m.distribution.is_none()
        && publishers
            .as_object()
            .into_iter()
            .flat_map(|keys| keys.keys())
            .any(|publisher| m.id.starts_with(&format!("{publisher}_")))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Registered publisher namespaces require signed packages".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{message, unsigned_namespace, verify};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ring::signature::KeyPair;
    use serde_json::json;
    use veyndo_app_model::distribution::Distribution;
    use veyndo_app_model::manifest::Manifest;
    #[test]
    fn publisher_signature_covers_permissions_and_channel() {
        let seed = [7u8; 32];
        let key = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let mut m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        m.id = "fixture_engraving".into();
        m.distribution = Some(Distribution {
            publisher: "fixture".into(),
            key_id: "first".into(),
            signature: String::new(),
            channel: "stable".into(),
            dependencies: vec![],
        });
        m.distribution.as_mut().unwrap().signature =
            STANDARD.encode(key.sign(&message(&m)).as_ref());
        let keys = json!({"fixture":{"first":STANDARD.encode(key.public_key().as_ref())}});
        assert!(verify(&m, &keys).is_ok());
        m.permissions.push("customers.pii".into());
        assert!(verify(&m, &keys).is_err());
        m.permissions.pop();
        m.distribution.as_mut().unwrap().channel = "beta".into();
        assert!(verify(&m, &keys).is_err());
        assert!(verify(&m, &json!({})).is_err());
    }
    #[test]
    fn an_unsigned_package_cannot_use_a_registered_publisher_namespace() {
        let mut m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        ))
        .unwrap();
        let publishers = json!({"fixture":{"first":"key"}});
        assert!(unsigned_namespace(&m, &publishers).is_ok());
        m.id = "fixture_engraving".into();
        assert!(unsigned_namespace(&m, &publishers).is_err());
        assert!(unsigned_namespace(&m, &json!({})).is_ok());
    }
}
