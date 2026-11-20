//! Namespaced core-object extension contracts; native records, rights and app storage keep their owners.
use crate::domain::model_contract;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::{Entity, Object};
use veyndo_core::{RequestContext, Result};
use veyndo_identity::permit;
use veyndo_kernel::verified_kernel;

/// Checks the object extensions a package declares.
pub fn validate(m: &Manifest) -> Result<()> {
    veyndo_app_model::object_extensions::validate(&m.entities, &m.actions, &m.permissions)
        .map_err(model_contract::error)
}

/// Requires the rights of the native object an entity extends.
pub fn access(e: &Entity, h: &RequestContext, write: bool) -> Result<()> {
    if let Some(x) = &e.extends {
        for scope in x.object.scopes(write) {
            permit(h, scope)?;
        }
    }
    Ok(())
}

/// Whether the records of an entity are release configuration rather than operational data.
pub fn configuration(e: &Entity) -> bool {
    let object = e.extends.as_ref().map(|x| x.object);
    verified_kernel::app_configuration_release_admissible(
        e.extends.is_some(),
        object == Some(Object::Product),
        object == Some(Object::Company),
        e.operational == Some(true),
    )
}

/// Whether the next version of an entity keeps the stored records of the old one valid.
pub fn compatible(old: &Entity, next: &Entity) -> Result<()> {
    veyndo_app_model::evolution::compatible(old, next).map_err(model_contract::error)
}

#[cfg(test)]
mod tests {
    use super::{access, compatible, configuration, validate};
    use serde_json::json;
    use veyndo_app_model::Object;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::RequestContext;
    fn package(object: Object) -> Manifest {
        serde_json::from_value(json!({
            "id":"native_notes","version":"1.0.0","coreApi":"1","runtime":"declarative",
            "name":{"en-GB":"Notes"},"permissions":["data.read","data.write"],
            "entities":[{"name":"notes","extends":{"version":1,"object":object,"field":"owner"},
                "fields":[{"name":"owner","kind":"string","required":true,"indexed":true,"unique":true,"coreReference":object.reference()}]}],
            "actions":[{"name":"read","description":"Read","handler":"list","entity":"notes","inputSchema":{"type":"object"},"requiredPermissions":object.scopes(false)},
                {"name":"write","description":"Write","handler":"save","entity":"notes","inputSchema":{"type":"object"},"requiredPermissions":object.scopes(true)}]
        })).unwrap()
    }
    #[test]
    fn native_bindings_need_native_rights_and_unique_private_anchors() {
        for object in [
            Object::Product,
            Object::Customer,
            Object::Order,
            Object::Company,
        ] {
            let m = package(object);
            assert!(validate(&m).is_ok());
            let mut unsafe_m = m.clone();
            unsafe_m.entities[0].fields[0].unique = false;
            assert!(validate(&unsafe_m).is_err());
            unsafe_m = m.clone();
            unsafe_m.entities[0].public_read = true;
            assert!(validate(&unsafe_m).is_err());
            unsafe_m = m.clone();
            unsafe_m.actions[0].required_permissions.clear();
            assert!(validate(&unsafe_m).is_err());
            unsafe_m = m.clone();
            unsafe_m.entities[0].extends = None;
            assert!(compatible(&m.entities[0], &unsafe_m.entities[0]).is_err());
        }
    }
    #[test]
    fn old_packages_keep_their_serialized_identity() {
        let mut m = package(Object::Product);
        m.entities[0].extends = None;
        assert!(
            !serde_json::to_value(&m).unwrap()["entities"][0]
                .as_object()
                .unwrap()
                .contains_key("extends")
        );
        assert!(
            serde_json::to_value(&m).unwrap()["entities"][0]
                .get("operational")
                .is_none()
        );
    }
    #[test]
    fn operational_receipts_do_not_become_release_configuration() {
        let mut m = package(Object::Product);
        let original = m.entities[0].clone();
        m.entities[0].operational = Some(true);
        assert!(!configuration(&m.entities[0]));
        assert!(compatible(&m.entities[0], &original).is_err());
        assert!(compatible(&original, &m.entities[0]).is_ok());
    }
    #[test]
    fn an_extension_needs_the_rights_of_its_native_object() {
        let m = package(Object::Order);
        let mut h = RequestContext::new();
        assert!(access(&m.entities[0], &h, false).is_err());
        h.insert("x-rac-role", "admin".parse().unwrap());
        assert!(access(&m.entities[0], &h, true).is_ok());
        let mut plain = m.entities[0].clone();
        plain.extends = None;
        assert!(access(&plain, &RequestContext::new(), true).is_ok());
    }
}
