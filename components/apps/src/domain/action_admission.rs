//! Native authenticated-current-rights adapter for the shared action and callback grant rules.
use crate::domain::model_contract;
use serde_json::Value;
use veyndo_app_model::Action;
use veyndo_core::{Error, RequestContext, Result};
use veyndo_identity::{merchant, permit};

/// The rights of the current actor, including the merchant scope.
pub struct Authority<'a>(pub &'a RequestContext);
impl veyndo_app_model::admission::ScopeAuthority for Authority<'_> {
    type Error = Error;
    fn require_scope(&self, scope: &str) -> Result<()> {
        permit(self.0, scope)
    }
}
impl veyndo_app_model::admission::Authority for Authority<'_> {
    fn require_merchant(&self) -> Result<()> {
        merchant(self.0).map(|_| ())
    }
}
/// The scopes of the current actor only.
pub struct Current<'a>(pub &'a RequestContext);
impl veyndo_app_model::admission::ScopeAuthority for Current<'_> {
    type Error = Error;
    fn require_scope(&self, scope: &str) -> Result<()> {
        permit(self.0, scope)
    }
}
/// The native error of a shared admission failure.
pub fn error(error: veyndo_app_model::admission::AdmissionError<Error>) -> Error {
    match error {
        veyndo_app_model::admission::AdmissionError::Authority(error) => error,
        veyndo_app_model::admission::AdmissionError::Model(error) => model_contract::error(error),
    }
}
/// Admits an action call of the current actor with its input.
pub fn check(h: &RequestContext, action: &Action, value: &Value) -> Result<()> {
    veyndo_app_model::admission::check(&Authority(h), action, value).map_err(error)
}

/// Additional current team rights apply equally to UI, HTTP, MCP, Flow and leased workers.
pub fn additional(h: &RequestContext, action: &Action) -> Result<()> {
    veyndo_app_model::admission::additional(&Current(h), action)
}
/// Whether the current actor may call an action; the registry uses the same check as direct calls.
pub fn authorized(h: &RequestContext, action: &Action) -> bool {
    action.public
        || merchant(h).is_ok()
            && additional(h, action).is_ok()
            && (!(["save", "service", "emit", "job"].contains(&action.handler.as_str())
                || action.permission.is_some())
                || permit(
                    h,
                    action
                        .permission
                        .as_deref()
                        .unwrap_or(if action.handler == "job" {
                            "apps.manage"
                        } else {
                            "catalog"
                        }),
                )
                .is_ok())
}

#[cfg(test)]
mod tests {
    use super::{additional, authorized, check, error};
    use crate::domain::approval::built_in;
    use serde_json::json;
    use veyndo_app_model::admission::AdmissionError;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_core::{RequestContext, StatusCode, bad};

    #[test]
    fn an_action_needs_the_rights_of_the_current_actor() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let action = m
            .actions
            .iter()
            .find(|action| !action.public)
            .unwrap_or(&m.actions[0]);
        let anonymous = RequestContext::new();
        let mut admin = RequestContext::new();
        admin.insert("x-rac-role", "admin".parse().unwrap());
        admin.insert("x-rac-tenant", "demo".parse().unwrap());
        admin.insert("x-tenant", "demo".parse().unwrap());
        let allowed = check(&admin, action, &json!({})).is_ok();
        let refused = check(&anonymous, action, &json!({})).is_ok();
        assert!(allowed || !refused);
        assert_eq!(
            error(AdmissionError::Authority(bad("refused"))).0,
            StatusCode::BAD_REQUEST
        );
    }
    #[test]
    fn a_public_action_is_open_and_a_private_one_needs_the_merchant() {
        let m: Manifest = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        let mut action = m.actions[0].clone();
        let anonymous = RequestContext::new();
        let mut admin = RequestContext::new();
        admin.insert("x-rac-role", "admin".parse().unwrap());
        admin.insert("x-rac-tenant", "demo".parse().unwrap());
        admin.insert("x-tenant", "demo".parse().unwrap());
        action.public = true;
        assert!(authorized(&anonymous, &action));
        action.public = false;
        action.handler = "save".into();
        action.permission = None;
        action.required_permissions.clear();
        assert!(!authorized(&anonymous, &action));
        assert!(authorized(&admin, &action));
        assert!(additional(&admin, &action).is_ok());
        action.required_permissions = vec!["orders.write".into()];
        assert_eq!(
            additional(&anonymous, &action).is_ok(),
            authorized(&anonymous, &action)
        );
    }
}
