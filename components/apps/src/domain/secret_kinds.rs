//! The kinds of per-shop secrets and which package declarations call for them.
use veyndo_app_model::manifest::Manifest;

/// The secrets a package may hold per shop.
pub const KINDS: [&str; 3] = ["service", "webhook", "outbound"];
/// Whether a package declares the use a secret kind serves.
pub fn applicable(m: &Manifest, kind: &str) -> bool {
    match kind {
        "service" => m.permissions.contains(&"service.call".into()),
        "webhook" => !m.webhooks.is_empty(),
        "outbound" => m.event_delivery.as_ref().is_some_and(|d| d.url.is_some()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{KINDS, applicable};
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn a_kind_applies_only_to_what_the_package_declares() {
        let mut m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        m.permissions.clear();
        m.webhooks.clear();
        m.event_delivery = None;
        assert!(KINDS.iter().all(|kind| !applicable(&m, kind)));
        m.permissions.push("service.call".into());
        assert!(applicable(&m, "service"));
        assert!(!applicable(&m, "webhook"));
        assert!(!applicable(&m, "settings"));
    }
}
