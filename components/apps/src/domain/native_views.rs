//! Shared view contracts of a package; actual grants, runtime execution and current rights remain with the server.
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;

fn contract(m: &Manifest) -> veyndo_app_model::views::Contract<'_> {
    veyndo_app_model::views::Contract {
        entities: &m.entities,
        actions: &m.actions,
        surfaces: &m.surfaces,
        views: &m.views,
    }
}
/// Whether a surface is rendered by the native view runtime.
pub fn is_native(m: &Manifest, s: &Surface) -> bool {
    veyndo_app_model::views::is_native(&contract(m), s)
}
/// The native view a surface renders, with the declarations it needs.
pub fn payload(m: &Manifest, s: &Surface) -> Option<Value> {
    veyndo_app_model::views::payload(&contract(m), s)
}

#[cfg(test)]
mod tests {
    use super::{is_native, payload};
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn a_native_surface_has_a_payload_and_a_hosted_one_none() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/care-studio/manifest.json"
        ))
        .unwrap();
        assert!(is_native(&m, &m.surfaces[0]));
        assert!(payload(&m, &m.surfaces[0]).is_some());
        let hosted: Manifest =
            serde_json::from_str(crate::domain::approval::built_in("slack").unwrap()).unwrap();
        for s in &hosted.surfaces {
            assert_eq!(is_native(&hosted, s), payload(&hosted, s).is_some());
        }
    }
}
