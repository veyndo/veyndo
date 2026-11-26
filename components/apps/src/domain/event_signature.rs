//! Outgoing app events carry a stable event id and an HMAC-SHA-256 over the shop, app, time, id and body digest.
use hmac::{Hmac, Mac};
use sha2::Sha256;
use veyndo_core::{Result, bad, hash};

/// The event id and hex signature of one outgoing body.
pub fn sign(
    secret: &str,
    t: &str,
    app: &str,
    bytes: &[u8],
    stamp: &str,
) -> Result<(String, String)> {
    let key = hash(&format!(
        "{t}:{app}:{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let canonical = format!(
        "{t}\n{app}\n{stamp}\n{key}\n{}",
        hash(std::str::from_utf8(bytes).map_err(|_| bad("Invalid event envelope"))?)
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| bad("Invalid signing secret"))?;
    mac.update(canonical.as_bytes());
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok((key, signature))
}

#[cfg(test)]
mod tests {
    use super::sign;
    use crate::domain::webhook_signature::verify;
    use veyndo_core::hash;

    #[test]
    fn the_signature_binds_shop_app_time_and_body() {
        let secret = "synthetic-test-secret-at-least-32-bytes";
        let body = br#"{"eventId":1}"#;
        let (key, signature) = sign(secret, "shop", "app", body, "1700000000").unwrap();
        let canonical = format!(
            "shop\napp\n1700000000\n{key}\n{}",
            hash(std::str::from_utf8(body).unwrap())
        );
        assert!(verify(secret, &canonical, &signature).is_ok());
        assert_eq!(
            sign(secret, "shop", "app", body, "1700000000").unwrap().0,
            key
        );
        assert_ne!(
            sign(secret, "other", "app", body, "1700000000").unwrap().0,
            key
        );
        assert_ne!(
            sign(secret, "shop", "app", body, "1700000001").unwrap().1,
            signature
        );
    }
}
