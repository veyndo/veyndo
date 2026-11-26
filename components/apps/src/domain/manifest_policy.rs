//! Required native policy ports for the complete shared compiler; no package or request fields can select a weaker policy.
use crate::domain::reference_policy::ReferencePolicy;
use crate::domain::{action_policy, distribution, event_destination, field_values};
use serde_json::Value;
use veyndo_app_model as shared;
use veyndo_app_model::Action;
use veyndo_app_model::manifest::Manifest;
use veyndo_core::localized::valid_locale_key;
use veyndo_core::{Error, Result};
use veyndo_identity::SCOPES;
use veyndo_pricing::types::currency as currencies;

/// The native policy of the package compiler, with the publisher keys the operator registered.
pub struct Policy<'a> {
    pub publishers: &'a Value,
}
impl shared::fields::ValuePolicy for Policy<'_> {
    fn locale(&self, key: &str) -> bool {
        shared::fields::ValuePolicy::locale(&field_values::Policy, key)
    }
    fn money(&self, value: &Value) -> bool {
        shared::fields::ValuePolicy::money(&field_values::Policy, value)
    }
    fn richtext(&self, value: &Value) -> bool {
        shared::fields::ValuePolicy::richtext(&field_values::Policy, value)
    }
    fn relations(&self, value: &Value) -> bool {
        shared::fields::ValuePolicy::relations(&field_values::Policy, value)
    }
}
impl shared::declarations::ReferencePolicy for Policy<'_> {
    fn admitted(&self, product: bool, private: bool) -> bool {
        shared::declarations::ReferencePolicy::admitted(&ReferencePolicy, product, private)
    }
}
impl shared::configuration::Policy for Policy<'_> {
    fn known_currency(&self, code: &str) -> bool {
        currencies::scale(code).is_some()
    }
}
impl shared::package::Policy for Policy<'_> {
    fn known_scope(&self, scope: &str) -> bool {
        SCOPES.contains(&scope)
    }
    fn component_source(&self, source: &str) -> bool {
        veyndo_apps::sandbox::Sandbox::component_source(source)
    }
}
impl shared::views::Policy for Policy<'_> {
    fn public_read(&self, writes: bool) -> bool {
        shared::views::Policy::public_read(&field_values::Policy, writes)
    }
}
impl shared::native_lifecycle::CompilerPolicy for Policy<'_> {
    fn scopes(&self) -> &[&str] {
        SCOPES
    }
    fn locale(&self, key: &str) -> bool {
        valid_locale_key(key)
    }
    fn order_base(&self) -> shared::native_lifecycle::Machine {
        veyndo_orders::domain::order_machine::default_machine()
    }
}
impl shared::distribution::SignatureVerifier for Policy<'_> {
    type Error = Error;
    fn verify_signature(&self, m: &Manifest) -> Result<()> {
        shared::distribution::SignatureVerifier::verify_signature(
            &distribution::NativeVerifier(self.publishers),
            m,
        )
    }
}
impl shared::compiler::Policy for Policy<'_> {
    fn unsigned_namespace(&self, m: &Manifest) -> Result<()> {
        distribution::unsigned_namespace(m, self.publishers)
    }
    fn destination(&self, url: &str) -> Result<()> {
        event_destination::destination(url).map(|_| ())
    }
    fn readonly(&self, action: &Action) -> bool {
        action_policy::readonly(action)
    }
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}
