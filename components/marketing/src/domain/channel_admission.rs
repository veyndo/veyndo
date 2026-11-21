//! Who may open a sales channel, which locales it serves and which products its selection holds.
use crate::types::channel::Channel;
use veyndo_core::{Error, Result, StatusCode};

/// Whether a channel may be opened by this request: marketplaces need connector access, POS a cashier, a paused channel its preview.
pub fn admissible(c: &Channel, id: &str, cashier: bool, preview: Option<&str>) -> Result<()> {
    if c.kind == "marketplace" {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Marketplace channels require connector access".into(),
        ));
    }
    if c.kind == "pos" && !cashier {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "POS requires authenticated cashier admission".into(),
        ));
    }
    if !c.active && preview != Some(id) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Sales channel or language unavailable".into(),
        ));
    }
    Ok(())
}

/// Whether a channel serves a locale; the main channel also accepts a regional variant of an enabled language.
pub fn serves(c: &Channel, id: &str, locale: &str) -> bool {
    c.locales.contains(&locale.to_string())
        || (id == "default"
            && c.locales
                .iter()
                .any(|enabled| enabled.split('-').next() == locale.split('-').next()))
}

/// Whether a product, or the parent it inherits from, is in a channel's product selection; an empty selection holds everything.
pub fn selected(selection: &[String], id: &str, parent: Option<&str>) -> bool {
    selection.is_empty()
        || selection.iter().any(|s| s == id)
        || parent.is_some_and(|p| selection.iter().any(|s| s == p))
}

#[cfg(test)]
mod tests {
    use super::{admissible, selected, serves};
    use crate::types::channel::Channel;
    use serde_json::json;
    use veyndo_core::StatusCode;

    fn channel(kind: &str, active: bool) -> Channel {
        serde_json::from_value(json!({"name":{"en-GB":"Shop"},"kind":kind,"active":active,"locales":["de-DE"],"productIds":[]})).unwrap()
    }

    #[test]
    fn marketplaces_cashiers_and_paused_channels_have_their_own_admission() {
        assert!(admissible(&channel("storefront", true), "web", false, None).is_ok());
        let refused = |c: &Channel, cashier, preview| {
            admissible(c, "web", cashier, preview).err().map(|e| e.0)
        };
        assert_eq!(
            refused(&channel("marketplace", true), true, None),
            Some(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            refused(&channel("pos", true), false, None),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(admissible(&channel("pos", true), "web", true, None).is_ok());
        assert_eq!(
            refused(&channel("storefront", false), false, None),
            Some(StatusCode::FORBIDDEN)
        );
        assert!(admissible(&channel("storefront", false), "web", false, Some("web")).is_ok());
        assert!(refused(&channel("storefront", false), false, Some("other")).is_some());
    }

    #[test]
    fn only_the_main_channel_accepts_a_regional_variant() {
        let c = channel("storefront", true);
        assert!(serves(&c, "web", "de-DE"));
        assert!(!serves(&c, "web", "de-AT"));
        assert!(serves(&c, "default", "de-AT"));
        assert!(!serves(&c, "default", "fr-FR"));
    }

    #[test]
    fn an_empty_selection_holds_every_product_and_variants_inherit_it() {
        assert!(selected(&[], "lamp", None));
        let selection = ["lamp".to_string()];
        assert!(selected(&selection, "lamp", None));
        assert!(selected(&selection, "lamp-red", Some("lamp")));
        assert!(!selected(&selection, "chair", None));
        assert!(!selected(&selection, "chair-red", Some("chair")));
    }
}
