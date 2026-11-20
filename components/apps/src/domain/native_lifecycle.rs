//! Versioned declarative native lifecycles; packages narrow native admission and never execute effects.
use crate::domain::{approval, model_contract};
use serde_json::Value;
use veyndo_app_model::manifest::Manifest;
use veyndo_app_model::native_lifecycle as shared;
pub use veyndo_app_model::native_lifecycle::{Snapshot, quote_machine};
use veyndo_core::localized::valid_locale_key;
use veyndo_core::{RequestContext, Result};
use veyndo_identity::{SCOPES, permit};
use veyndo_kernel::verified_kernel;
use veyndo_orders::domain::order_machine::{MachineEdge, OrderMachine, default_machine};

/// The native scope catalogue, locale rule and order base of the lifecycle compiler.
pub struct HostPolicy;
impl shared::CompilerPolicy for HostPolicy {
    fn scopes(&self) -> &[&str] {
        SCOPES
    }
    fn locale(&self, key: &str) -> bool {
        valid_locale_key(key)
    }
    fn order_base(&self) -> shared::Machine {
        default_machine()
    }
}
/// A package as the shared lifecycle compiler sees it.
pub fn package<'a>(m: &'a Manifest, digest: &'a str) -> shared::Package<'a> {
    shared::Package {
        id: &m.id,
        version: &m.version,
        digest,
        permissions: &m.permissions,
        lifecycles: &m.lifecycles,
    }
}
/// What a generator may use when it writes a lifecycle.
pub fn generation_context() -> Value {
    shared::generation_context(&HostPolicy)
}
/// Compiles the lifecycles of the active packages onto a native base machine.
pub fn compile(base: OrderMachine, apps: &[Manifest], object: &str) -> Result<OrderMachine> {
    let digests: Vec<_> = apps.iter().map(approval::canonical_digest).collect();
    let packages: Vec<_> = apps
        .iter()
        .zip(&digests)
        .map(|(m, d)| package(m, d))
        .collect();
    shared::compile(base, &packages, object, &HostPolicy).map_err(model_contract::error)
}
/// Checks the lifecycles a package declares.
pub fn validate(m: &Manifest) -> Result<()> {
    let digest = approval::canonical_digest(m);
    veyndo_app_model::native_lifecycle::validate(&package(m, &digest), &HostPolicy)
        .map_err(model_contract::error)
}
/// Requires labels in the main language of the shop.
pub fn validate_locale(m: &Manifest, main: &str) -> Result<()> {
    let digest = approval::canonical_digest(m);
    veyndo_app_model::native_lifecycle::validate_locale(&package(m, &digest), main)
        .map_err(model_contract::error)
}
/// Why a transition is refused for the current actor and facts, if it is.
pub fn reason(h: &RequestContext, edge: &MachineEdge, data: &Value) -> Option<&'static str> {
    veyndo_app_model::native_lifecycle::reason(edge, data, |p| permit(h, p).is_ok())
}
/// Whether a transition is admitted: natively allowed, with rights and guards.
pub fn allowed(native: bool, rights: bool, guards: bool) -> bool {
    verified_kernel::native_lifecycle_admissible(native, rights, guards)
}

/// The lifecycle an object was accepted under, if it carries one for this object.
pub fn snapshot(data: &Value, object: &str) -> Result<Option<Snapshot>> {
    veyndo_app_model::native_lifecycle_snapshot::snapshot(data, object)
        .map_err(model_contract::error)
}
/// The machine an order was accepted under, or the fallback.
pub fn order_machine(
    data: &Value,
    fallback: OrderMachine,
    revision: i64,
) -> Result<(OrderMachine, i64)> {
    veyndo_app_model::native_lifecycle_snapshot::order_machine(data, fallback, revision)
        .map_err(model_contract::error)
}

/// Copies the accepted labels of a lifecycle into a response.
pub fn project_labels(raw: &Value, output: &mut Value) {
    veyndo_app_model::native_lifecycle_snapshot::project_labels(raw, output)
}

#[cfg(test)]
mod tests {
    use super::{
        Snapshot, allowed, compile, generation_context, order_machine, package, project_labels,
        reason, snapshot, validate, validate_locale,
    };
    use crate::domain::approval;
    use serde_json::json;
    use veyndo_app_model::declarations::identifier;
    use veyndo_app_model::manifest::Manifest;
    use veyndo_app_model::native_lifecycle::fact_value;
    use veyndo_core::RequestContext;
    use veyndo_orders::domain::order_machine::{MachineState, default_machine};

    fn manifest() -> Manifest {
        serde_json::from_str(include_str!(
            "../../../../extensions/apps/production-review/manifest.json"
        ))
        .unwrap()
    }
    #[test]
    fn requires_native_labels_in_the_configured_main_language() {
        let mut m = manifest();
        validate_locale(&m, "de-DE").unwrap();
        assert!(validate_locale(&m, "it-IT").is_err());
        for c in &mut m.lifecycles {
            for s in &mut c.states {
                s.label.insert("it".into(), "Controllo".into());
            }
            for e in &mut c.transitions {
                e.label.insert("it".into(), "Prosegui".into());
            }
        }
        validate_locale(&m, "it-IT").unwrap();
        m.lifecycles[0].states[0]
            .label
            .insert("it-IT".into(), " ".into());
        assert!(validate_locale(&m, "it-IT").is_err());
    }
    #[test]
    fn package_local_graphs_cannot_replace_payment_or_buyer_acceptance() {
        let m = manifest();
        validate(&m).unwrap();
        for target in ["offered", "accepted", "ordered"] {
            let mut invalid = m.clone();
            invalid.lifecycles[1].transitions[1].to = target.into();
            assert!(validate(&invalid).is_err());
        }
        let mut special = default_machine();
        special.states.push(MachineState {
            id: "manual_archive".into(),
            label: m.lifecycles[0].states[0].label.clone(),
            terminal: true,
        });
        let mut invalid = m.clone();
        invalid.lifecycles[0].transitions[1].to = "manual_archive".into();
        assert!(compile(special, &[invalid], "order").is_err());
        for replace in ["cancel_new", "payments.capture", "unknown"] {
            let mut invalid = m.clone();
            invalid.lifecycles[0].transitions[0].replaces = Some(replace.into());
            assert!(validate(&invalid).is_err());
        }
        let mut invalid = m.clone();
        invalid.lifecycles[0].transitions[1].from = "completed".into();
        assert!(validate(&invalid).is_err());
    }
    #[test]
    fn install_cannot_compete_strand_or_execute_unbounded_guards() {
        let m = manifest();
        let mut other = m.clone();
        other.id = "competitor".into();
        assert!(compile(default_machine(), &[m.clone(), other], "order").is_err());
        let mut invalid = m.clone();
        invalid.lifecycles[0].transitions.truncate(1);
        assert!(validate(&invalid).is_err());
        let mut invalid = m.clone();
        invalid.lifecycles[0].transitions[1]
            .guards
            .push("sql:select".into());
        assert!(validate(&invalid).is_err());
        let mut invalid = m.clone();
        invalid.lifecycles[0].transitions[1]
            .required_permissions
            .push("owner".into());
        assert!(validate(&invalid).is_err());
        let mut invalid = m.clone();
        invalid.permissions.clear();
        assert!(validate(&invalid).is_err());
    }
    #[test]
    fn namespaced_graph_and_receipt_keep_exact_accepted_version() {
        let m = manifest();
        let machine = compile(default_machine(), std::slice::from_ref(&m), "order").unwrap();
        assert!(!machine.transitions.iter().any(|e| e.id == "complete"));
        let e = machine
            .transitions
            .iter()
            .find(|e| e.id.ends_with("_approve"))
            .unwrap();
        assert_eq!(
            e.policy.as_ref().unwrap().digest,
            approval::canonical_digest(&m)
        );
        assert_eq!(e.policy.as_ref().unwrap().version, "1.0.0");
        assert!(identifier(&e.from));
        assert!(identifier(&e.id));
        let order = json!({"nativeLifecycle":Snapshot {version:1,object:"order".into(),revision:3,main_locale:"it-IT".into(),machine}});
        assert_eq!(
            snapshot(&order, "order").unwrap().unwrap().main_locale,
            "it-IT"
        );
        assert!(snapshot(&order, "quote").is_err());
        assert!(snapshot(&json!({}), "order").unwrap().is_none());
    }
    #[test]
    fn app_guards_only_narrow_and_never_invent_payment_or_delivery_truth() {
        for payment in ["pending", "authorized", "paid", "captured", "refunded"] {
            let data = json!({"payment":{"provider":"manual","state":payment},"deliveries":[{"state":"open"}]});
            assert_eq!(
                fact_value("payment.ready", &data),
                ["paid", "captured"].contains(&payment)
            );
            assert!(!fact_value("delivery.delivered", &data));
            assert!(!allowed(false, true, true));
            assert!(!allowed(true, false, true));
        }
        assert!(!fact_value("unknown", &json!({})));
        assert!(!fact_value("quote.message", &json!({"message":"  "})));
        assert!(fact_value(
            "quote.message",
            &json!({"message":"Material check"})
        ));
    }

    #[test]
    fn an_order_without_a_snapshot_keeps_the_fallback_machine_and_generators_see_scopes() {
        let (machine, revision) = order_machine(&json!({}), default_machine(), 4).unwrap();
        assert_eq!(revision, 4);
        assert_eq!(machine.states.len(), default_machine().states.len());
        let mut output = json!({});
        project_labels(&json!({}), &mut output);
        assert!(generation_context().is_object());
    }

    #[test]
    fn a_transition_names_the_right_an_actor_lacks() {
        let m = manifest();
        let machine = compile(default_machine(), std::slice::from_ref(&m), "order").unwrap();
        let edge = machine
            .transitions
            .iter()
            .find(|e| e.id.ends_with("_approve"))
            .unwrap();
        let anonymous = RequestContext::new();
        assert!(reason(&anonymous, edge, &json!({})).is_some());
        let described = package(&m, "digest");
        assert_eq!(described.id, m.id);
        assert_eq!(described.digest, "digest");
        assert_eq!(described.lifecycles.len(), m.lifecycles.len());
    }
}
