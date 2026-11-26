//! Complete shared package compiler adapter; trusted native policies retain current authority and exact native errors.
use crate::domain::manifest_policy::Policy;
use crate::domain::model_contract;
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::package::CompileError;
use veyndo_core::Result;

/// Compiles a package under the native policy and the operator's publisher keys.
pub fn validate(m: &Manifest, publishers: &Value) -> Result<()> {
    veyndo_app_model::compiler::compile(m, &Policy { publishers }).map_err(|e| match e {
        CompileError::Host(e) => e,
        CompileError::Model(e) => model_contract::error(e),
    })
}
#[cfg(test)]
mod tests {
    use super::validate as compile;
    use serde_json::{Value, json};
    use veyndo_app_model::manifest::Manifest;

    fn validate(m: &Manifest) -> veyndo_core::Result<()> {
        compile(m, &json!({}))
    }
    #[test]
    fn extra_action_rights_are_known_private_scopes() {
        let mut value: Value = serde_json::from_str(include_str!(
            "../../../../extensions/apps/marketplaces/manifest.json"
        ))
        .unwrap();
        let parse = |v: &Value| serde_json::from_value::<Manifest>(v.clone()).unwrap();
        assert!(validate(&parse(&value)).is_ok());
        let idx = value["actions"]
            .as_array()
            .unwrap()
            .iter()
            .position(|a| a["name"] == "import_order")
            .unwrap();
        value["actions"][idx]["requiredPermissions"] = json!(["not.a.scope"]);
        assert!(validate(&parse(&value)).is_err());
        value["actions"][idx]["requiredPermissions"] = json!(["customers.pii"]);
        value["actions"][idx]["public"] = json!(true);
        value["actions"][idx]
            .as_object_mut()
            .unwrap()
            .remove("permission");
        assert!(validate(&parse(&value)).is_err());
    }
    #[test]
    fn an_unsigned_package_in_a_publisher_namespace_does_not_compile() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/marketplaces/manifest.json"
        ))
        .unwrap();
        assert!(compile(&m, &json!({"other":{"k":"key"}})).is_ok());
        let prefix = m.id.split('_').next().unwrap_or(&m.id).to_owned();
        let mut scoped = m;
        scoped.id = format!("{prefix}_app");
        assert!(compile(&scoped, &json!({prefix:{"k":"key"}})).is_err());
    }
}
