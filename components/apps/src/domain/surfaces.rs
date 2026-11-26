//! App-owned UI surfaces and namespaced routes: their contract and where an operator-configured UI is served from.
use crate::domain::action_policy::readonly;
use crate::domain::service_ui::ui_url;
use crate::domain::{model_contract, native_views};
use serde_json::Value;
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::surfaces::Surface;
use veyndo_core::Result;
use veyndo_identity::SCOPES;

struct SurfacePolicy<'a>(&'a Manifest);
impl veyndo_app_model::surfaces::Policy for SurfacePolicy<'_> {
    fn native_view(&self, surface: &Surface) -> bool {
        native_views::is_native(self.0, surface)
    }
    fn assistant_view(&self, surface: &Surface) -> Option<&veyndo_app_model::views::NativeView> {
        self.0
            .views
            .iter()
            .find(|view| surface.ui_path == format!("native/{}", view.id))
    }
    fn known_scope(&self, scope: &str) -> bool {
        SCOPES.contains(&scope)
    }
    fn readonly(&self, action: &Action) -> bool {
        readonly(action)
    }
}
/// Checks the surfaces, routes and agent tools a package declares.
pub fn validate_contract(m: &Manifest) -> Result<()> {
    let contract = veyndo_app_model::surfaces::Contract {
        runtime: &m.runtime,
        permissions: &m.permissions,
        actions: &m.actions,
        surfaces: &m.surfaces,
        api_routes: &m.api_routes,
    };
    veyndo_app_model::surfaces::validate(&contract, &SurfacePolicy(m))
        .map_err(model_contract::error)?;
    veyndo_app_model::intelligence::validate(m.intelligence.as_ref(), &m.actions, &m.entities)
        .map_err(model_contract::error)?;
    Ok(())
}
/// The address of one UI path below the operator-configured UI of an app.
pub fn surface_url(id: &str, path: &str, services: &Value) -> Option<String> {
    let mut url = reqwest::Url::parse(&ui_url(id, services)?).ok()?;
    let base = url.path().trim_end_matches('/');
    let next = format!("{base}/{path}");
    url.set_path(&next);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}
#[cfg(test)]
mod tests {
    use super::{SurfacePolicy, surface_url, validate_contract};
    use serde_json::json;
    use veyndo_app_model::Action;
    use veyndo_app_model::manifest::Manifest;
    #[test]
    fn native_surface_policy_retains_verified_effect_guards() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/care-studio/manifest.json"
        ))
        .unwrap();
        let mut action: Action = serde_json::from_value(json!({"name":"effect","description":"Effect","handler":"save","readOnly":true,"inputSchema":{"type":"object"}})).unwrap();
        let policy = SurfacePolicy(&m);
        for handler in ["save", "emit", "submit"] {
            action.handler = handler.into();
            assert!(!veyndo_app_model::surfaces::Policy::readonly(
                &policy, &action
            ));
        }
        action.read_only = false;
        for handler in ["list", "configurations"] {
            action.handler = handler.into();
            assert!(veyndo_app_model::surfaces::Policy::readonly(
                &policy, &action
            ));
        }
    }
    #[test]
    fn a_surface_address_stays_below_the_configured_ui_without_query_or_fragment() {
        let services = json!({"sample":{"uiUrl":"https://apps.example/ui/?token=1#x"}});
        assert_eq!(
            surface_url("sample", "editor.html", &services).as_deref(),
            Some("https://apps.example/ui/editor.html")
        );
        assert_eq!(surface_url("missing", "editor.html", &services), None);
    }

    #[test]
    fn a_bundled_package_has_a_valid_surface_contract() {
        let m: Manifest = serde_json::from_str(include_str!(
            "../../../../extensions/apps/care-studio/manifest.json"
        ))
        .unwrap();
        assert!(validate_contract(&m).is_ok());
    }
}
