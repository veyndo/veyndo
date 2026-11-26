//! How many events one delivery to an app may carry.
use veyndo_app_model::manifest::Manifest;

/// The declared batch size of a package; one event when it declares none.
pub fn batch_size(m: &Manifest) -> i64 {
    m.event_delivery
        .as_ref()
        .map(|d| d.batch_size as i64)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::batch_size;
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn a_package_without_delivery_settings_receives_single_events() {
        let mut m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        m.event_delivery = None;
        assert_eq!(batch_size(&m), 1);
    }
}
