//! Native error and Manifest adapters for the shared pure evolution planner; transactional execution uses shared schema_upgrade through required native validation.
use crate::domain::model_contract;
use veyndo_app_model::Entity;
pub use veyndo_app_model::evolution::Migration;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::Result;

/// The entities after an upgrade, or why the new package cannot follow the old one.
pub fn shape(old: &Manifest, next: &Manifest, path: Option<&Migration>) -> Result<Vec<Entity>> {
    veyndo_app_model::evolution::shape(&old.entities, &next.entities, path)
        .map_err(model_contract::error)
}

#[cfg(test)]
mod tests {
    use super::shape;
    use crate::domain::approval::built_in;
    use veyndo_app_model::manifest::Manifest;

    #[test]
    fn an_unchanged_package_keeps_its_entities_and_a_removed_one_needs_a_migration() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        assert!(!m.entities.is_empty());
        let same = shape(&m, &m, None).unwrap();
        assert_eq!(
            same.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            m.entities
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
        );
        let mut removed = serde_json::from_str::<Manifest>(built_in("engraving").unwrap()).unwrap();
        removed.entities.clear();
        assert!(shape(&m, &removed, None).is_err());
    }
}
