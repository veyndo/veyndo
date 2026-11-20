//! Operator service authority is bound to immutable package content, never to a merchant-chosen ID.
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::{Error, Result, StatusCode};

/// The bundled manifest of a first-party app.
pub fn built_in(id: &str) -> Option<&'static str> {
    match id {
        "site_builder" => Some(include_str!(
            "../../../../extensions/apps/site-builder/manifest.json"
        )),
        "site_forms" => Some(include_str!(
            "../../../../extensions/apps/site-forms/manifest.json"
        )),
        "migration" => Some(include_str!(
            "../../../../extensions/apps/migration/manifest.json"
        )),
        "marketplaces" => Some(include_str!(
            "../../../../extensions/apps/marketplaces/manifest.json"
        )),
        "engraving" => Some(include_str!(
            "../../../../extensions/apps/engraving/manifest.json"
        )),
        "storyfront" => Some(include_str!(
            "../../../../extensions/apps/storyfront/manifest.json"
        )),
        "paypal" => Some(include_str!(
            "../../../../extensions/apps/paypal/manifest.json"
        )),
        "shopware_payments" => Some(include_str!(
            "../../../../extensions/apps/shopware-payments/manifest.json"
        )),
        "google_analytics" => Some(include_str!(
            "../../../../extensions/apps/google-analytics/manifest.json"
        )),
        "gmail" => Some(include_str!(
            "../../../../extensions/apps/gmail/manifest.json"
        )),
        "email" => Some(include_str!(
            "../../../../extensions/apps/email/manifest.json"
        )),
        "slack" => Some(include_str!(
            "../../../../extensions/apps/slack/manifest.json"
        )),
        "vercel" => Some(include_str!(
            "../../../../extensions/apps/vercel/manifest.json"
        )),
        "klaviyo" => Some(include_str!(
            "../../../../extensions/apps/klaviyo/manifest.json"
        )),
        "hubspot" => Some(include_str!(
            "../../../../extensions/apps/hubspot/manifest.json"
        )),
        "contentful" => Some(include_str!(
            "../../../../extensions/apps/contentful/manifest.json"
        )),
        "algolia" => Some(include_str!(
            "../../../../extensions/apps/algolia/manifest.json"
        )),
        _ => None,
    }
}
/// The manifest a first-party app shipped with before it moved into its package folder.
pub fn legacy_built_in(id: &str) -> Option<&'static str> {
    match id {
        "engraving" => Some(include_str!(
            "../../../../reference/app-manifests/engraving.json"
        )),
        "storyfront" => Some(include_str!(
            "../../../../reference/app-manifests/storyfront.json"
        )),
        "paypal" => Some(include_str!(
            "../../../../reference/app-manifests/paypal.json"
        )),
        "shopware_payments" => Some(include_str!(
            "../../../../reference/app-manifests/shopware-payments.json"
        )),
        "google_analytics" => Some(include_str!(
            "../../../../reference/app-manifests/google-analytics.json"
        )),
        "gmail" => Some(include_str!(
            "../../../../reference/app-manifests/gmail.json"
        )),
        "email" => Some(include_str!(
            "../../../../reference/app-manifests/email.json"
        )),
        "slack" => Some(include_str!(
            "../../../../reference/app-manifests/slack.json"
        )),
        _ => None,
    }
}
/// The digest of the canonical package content.
pub fn canonical_digest(m: &Manifest) -> String {
    veyndo_app_model::manifest::canonical_digest(m)
}
/// Whether exactly this package content is pinned by the operator or bundled with the server.
pub fn approved(m: &Manifest, service: &Value) -> bool {
    let digest = canonical_digest(m);
    let pinned = service["approvedDigests"]
        .as_array()
        .is_some_and(|pins| pins.iter().any(|p| p.as_str() == Some(digest.as_str())));
    let previous = (m.id == "email").then_some(include_str!(
        "../../../../extensions/apps/email/manifest-1.2.0.json"
    ));
    let previous_payment = (m.id == "shopware_payments").then_some(include_str!(
        "../../../../extensions/apps/shopware-payments/manifest-1.5.0.json"
    ));
    let bundled = [
        built_in(&m.id),
        legacy_built_in(&m.id),
        previous,
        previous_payment,
    ]
    .into_iter()
    .flatten()
    .filter_map(|s| serde_json::from_str::<Manifest>(s).ok())
    .any(|original| canonical_digest(&original) == digest);
    veyndo_kernel::verified_kernel::app_package_authorized(pinned, bundled)
}
/// Refuses a package that claims a reserved app ID or an operator service without its approved content.
pub fn installation(m: &Manifest, services: &Value) -> Result<()> {
    let service = &services[&m.id];
    if (built_in(&m.id).is_some() || !service.is_null()) && !approved(m, service) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Package digest is not approved for this service or reserved app ID".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{approved, built_in, canonical_digest, installation, legacy_built_in};
    use serde_json::json;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::StatusCode;
    #[test]
    fn email_upgrade_keeps_exact_previous_package_without_inheriting_new_consent() {
        let mut m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/email/manifest-1.2.0.json"
        ))
        .unwrap();
        assert!(approved(&m, &json!({})));
        assert!(
            !m.permissions
                .iter()
                .any(|p| p == "payments.instructions.read")
        );
        m.permissions.push("payments.instructions.read".into());
        assert!(!approved(&m, &json!({})));
    }
    #[test]
    fn modified_reserved_payment_package_does_not_inherit_operator_authority() {
        let mut m: Manifest = serde_json::from_str(built_in("shopware_payments").unwrap()).unwrap();
        assert!(approved(&m, &json!({})));
        m.actions.clear();
        m.name.insert("en".into(), "Unapproved replacement".into());
        assert!(!approved(&m, &json!({})));
        assert!(approved(
            &m,
            &json!({"approvedDigests":[canonical_digest(&m)]})
        ));
        m.version = "9.0.0".into();
        assert!(!approved(
            &m,
            &json!({"approvedDigests":["not-the-current-digest"]})
        ));
    }
    #[test]
    fn installation_admits_unreserved_apps_and_only_approved_reserved_ones() {
        let mut m: Manifest = serde_json::from_str(built_in("slack").unwrap()).unwrap();
        assert!(installation(&m, &json!({})).is_ok());
        m.permissions.push("customers.pii".into());
        assert_eq!(
            installation(&m, &json!({})).err().map(|e| e.0),
            Some(StatusCode::FORBIDDEN)
        );
        m.id = "own_app".into();
        assert!(installation(&m, &json!({})).is_ok());
        assert!(installation(&m, &json!({"own_app":{"url":"https://service.example"}})).is_err());
        let pinned = json!({"own_app":{"approvedDigests":[canonical_digest(&m)]}});
        assert!(installation(&m, &pinned).is_ok());
    }
    #[test]
    fn earlier_manifests_are_kept_for_the_apps_that_shipped_them() {
        for id in [
            "engraving",
            "storyfront",
            "paypal",
            "shopware_payments",
            "google_analytics",
            "gmail",
            "email",
            "slack",
        ] {
            let m: Manifest = serde_json::from_str(legacy_built_in(id).unwrap()).unwrap();
            assert_eq!(m.id, id);
            assert!(built_in(id).is_some(), "{id}");
        }
        assert!(legacy_built_in("site_builder").is_none());
        assert!(legacy_built_in("unknown").is_none());
        assert!(built_in("unknown").is_none());
    }
}
