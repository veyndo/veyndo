//! The signed package contract beside the Manifest: per-tool planner facts, typed app-to-app ports and explicit rollback migrations. It describes a package for agents and reviewers; it never grants a right.
use crate::domain::action_policy::readonly;
use crate::domain::app_facts::{
    AppFacts, Audience, Condition, Data, Entity, Reversibility, Risk, SideEffect,
};
use crate::domain::approval::canonical_digest;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use veyndo_app_model::Action;
use veyndo_app_model::evolution::Step;
use veyndo_app_model::manifest::{Manifest, value_digest};
use veyndo_core::{Error, Result, StatusCode, bad};

/// The contract of one package version. Bundled apps keep it as `contract.json` next to their manifest.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contract {
    pub app: String,
    pub version: String,
    /// Declared planner facts by action name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolFacts>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<ProvidedPort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumes: Vec<ConsumedPort>,
    /// How to bring this version's app data back to an earlier version.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rollbacks: Vec<Rollback>,
    /// Ed25519 signature of the publisher named in the Manifest distribution, over [`message`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}
/// What a publisher declares about one tool, in the native facts vocabulary. Audience, permissions
/// and whether it writes come from the Manifest; the host adds what the handler implies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolFacts {
    pub reversibility: Reversibility,
    pub risk: Risk,
    #[serde(default)]
    pub data: Data,
    #[serde(default)]
    pub effects: Vec<SideEffect>,
    /// Operations whose result this tool consumes (native names or `app.<app>.<action>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}
/// A named, versioned interface this app serves through one of its actions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvidedPort {
    pub name: String,
    /// A semantic version of the interface.
    pub version: String,
    pub action: String,
    pub input_schema: Value,
    pub output_schema: Value,
}
/// A named interface this app calls, with the versions it accepts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsumedPort {
    pub name: String,
    /// A semantic version requirement such as `^1.2`.
    pub version: String,
}
/// The data steps that return this version's records to an earlier version.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rollback {
    pub to_version: String,
    pub steps: Vec<Step>,
}
impl std::fmt::Debug for Rollback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rollback({}, {} steps)",
            self.to_version,
            self.steps.len()
        )
    }
}
impl PartialEq for Rollback {
    fn eq(&self, other: &Self) -> bool {
        self.to_version == other.to_version
            && serde_json::to_value(&self.steps).ok() == serde_json::to_value(&other.steps).ok()
    }
}

/// A port name: lowercase, dotted, at most 64 characters (`acme.tax-quote`).
fn port_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
}
/// A semantic version, as packages, ports and rollbacks use it.
pub fn version(v: &str) -> Result<semver::Version> {
    semver::Version::parse(v).map_err(|_| bad("App package versions must be semantic versions"))
}
/// The exact bytes a publisher signs for a contract: it binds the canonical Manifest digest, so
/// neither side can be exchanged without the other.
pub fn message(m: &Manifest, c: &Contract) -> Vec<u8> {
    let mut unsigned = c.clone();
    unsigned.signature = None;
    let contract = serde_json::to_value(&unsigned).expect("Serializable contract");
    format!(
        "veyndo-app-contract/1\n{}\n{}",
        canonical_digest(m),
        value_digest(&contract)
    )
    .into_bytes()
}
/// The digest a review shows and an installation repeats.
pub fn digest(c: &Contract) -> String {
    value_digest(&serde_json::to_value(c).expect("Serializable contract"))
}

/// Checks a contract against its Manifest and, for a signed package, the publisher's signature.
pub fn validate(m: &Manifest, c: &Contract, publishers: &Value) -> Result<()> {
    if c.app != m.id || c.version != m.version {
        return Err(bad(
            "Package contract must name the same app and version as its manifest",
        ));
    }
    let own = version(&m.version)?;
    if c.tools.len() > 64 || c.provides.len() > 8 || c.consumes.len() > 8 || c.rollbacks.len() > 8 {
        return Err(bad("Package contract exceeds its declaration limits"));
    }
    for (name, declared) in &c.tools {
        let Some(action) = m.actions.iter().find(|a| &a.name == name) else {
            return Err(bad("Package contract describes an undeclared action"));
        };
        if declared.data.entities.len() > 16
            || declared.effects.len() > 7
            || declared.requires.len() > 16
        {
            return Err(bad("Invalid declared tool facts"));
        }
        facts(m, action, Some(c))
            .0
            .check(&tool_name(m, action))
            .map_err(|why| bad(format!("Contradictory tool facts: {why}")))?;
    }
    let mut names = HashSet::new();
    for port in &c.provides {
        version(&port.version)?;
        let action = m.actions.iter().find(|a| a.name == port.action);
        if !port_name(&port.name) || !names.insert(port.name.as_str()) {
            return Err(bad("Invalid or repeated provided port"));
        }
        let Some(action) = action else {
            return Err(bad("A provided port must name a declared action"));
        };
        if value_digest(&action.input_schema) != value_digest(&port.input_schema)
            || action.output_schema.as_ref().map(value_digest)
                != Some(value_digest(&port.output_schema))
        {
            return Err(bad(
                "A provided port's schemas must equal its action's input and output schemas",
            ));
        }
    }
    let mut consumed = HashSet::new();
    for port in &c.consumes {
        if !port_name(&port.name)
            || !consumed.insert(port.name.as_str())
            || names.contains(port.name.as_str())
            || semver::VersionReq::parse(&port.version).is_err()
        {
            return Err(bad("Invalid or repeated consumed port"));
        }
    }
    for rollback in &c.rollbacks {
        if version(&rollback.to_version)? >= own || rollback.steps.len() > 64 {
            return Err(bad("A rollback must lead to an earlier version"));
        }
    }
    signature(m, c, publishers)
}
/// A signed Manifest needs a contract signed by the same publisher key; an unsigned one carries none.
fn signature(m: &Manifest, c: &Contract, publishers: &Value) -> Result<()> {
    let forbidden = |message: &str| Error(StatusCode::FORBIDDEN, message.into());
    match (&m.distribution, &c.signature) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(bad("An unsigned package cannot carry a contract signature")),
        (Some(_), None) => Err(forbidden("A signed package requires a signed contract")),
        (Some(d), Some(signature)) => {
            let public = publishers[&d.publisher][&d.key_id]
                .as_str()
                .ok_or(forbidden("Publisher key is not registered by the operator"))?;
            let key = STANDARD
                .decode(public)
                .map_err(|_| bad("Invalid registered publisher public key"))?;
            let signature = STANDARD
                .decode(signature)
                .map_err(|_| bad("Invalid contract signature encoding"))?;
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
                .verify(&message(m, c), &signature)
                .map_err(|_| forbidden("Package contract signature does not match its content"))
        }
    }
}

/// The permissions a call of an action needs, as the gateway checks them; a public action needs none.
pub fn permissions(action: &Action) -> Vec<String> {
    let mut all = Vec::new();
    if action.public {
        return all;
    }
    if let Some(p) = &action.permission {
        all.push(p.clone());
    } else if ["save", "service", "emit", "job"].contains(&action.handler.as_str()) {
        all.push(
            if action.handler == "job" {
                "apps.manage"
            } else {
                "catalog"
            }
            .into(),
        );
    }
    for p in &action.required_permissions {
        if !all.contains(p) {
            all.push(p.clone());
        }
    }
    all
}
/// The MCP and planner name of an app tool.
pub fn tool_name(m: &Manifest, action: &Action) -> String {
    format!("app.{}.{}", m.id, action.name)
}
/// The facts agents plan with and whether the publisher declared them. The Manifest decides
/// audience, permissions and whether the tool writes; the handler adds the effects it always has
/// (a service call is external, a job queues work, an emit is an event). An undeclared writing tool
/// is irreversible and high risk, so it needs approval.
pub fn facts(m: &Manifest, action: &Action, c: Option<&Contract>) -> (AppFacts, bool) {
    let reads = readonly(action);
    let entity = action
        .entity
        .as_ref()
        .and_then(|n| m.entities.iter().find(|e| &e.name == n));
    let mut data = Data {
        entities: vec![Entity::App],
        ..Data::default()
    };
    if let Some(core) = entity.and_then(|e| e.extends.as_ref()) {
        use veyndo_app_model::Object;
        data.entities.push(match core.object {
            Object::Product => Entity::Product,
            Object::Customer => Entity::Customer,
            Object::Order => Entity::Order,
            Object::Company => Entity::Company,
        });
        data.pii = matches!(core.object, Object::Customer | Object::Order);
    }
    data.pii |= action
        .required_permissions
        .iter()
        .any(|p| p == "customers.pii");
    let implied: Vec<SideEffect> = match action.handler.as_str() {
        "service" => vec![SideEffect::ExternalCall],
        "job" => vec![SideEffect::BackgroundJob],
        "emit" => vec![SideEffect::Event],
        _ => vec![],
    };
    let declared = c.and_then(|c| c.tools.get(&action.name));
    let audience = if action.public {
        Audience::Shopper
    } else {
        Audience::Merchant
    };
    let mut f = AppFacts {
        audience,
        permissions: permissions(action),
        writes: !reads,
        reversibility: if reads {
            Reversibility::Reversible
        } else {
            Reversibility::Irreversible
        },
        risk: if reads { Risk::Low } else { Risk::High },
        data,
        effects: implied,
        requires: vec![],
        conditions: vec![],
    };
    if let Some(d) = declared {
        if !reads {
            f.reversibility = d.reversibility.clone();
            f.risk = d.risk;
        } else {
            f.risk = d.risk.min(Risk::Medium);
        }
        for e in &d.data.entities {
            if !f.data.entities.contains(e) {
                f.data.entities.push(*e);
            }
        }
        f.data.pii |= d.data.pii;
        f.data.money |= d.data.money;
        f.data.stock |= d.data.stock;
        f.effects.extend(d.effects.iter().copied());
        f.requires = d.requires.clone();
        f.conditions = d.conditions.clone();
    }
    (f.normalize(&tool_name(m, action)), declared.is_some())
}
/// A bundled package's contract, if it ships one.
pub fn built_in(id: &str) -> Option<&'static str> {
    BUILT_IN
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, c)| *c)
}
const BUILT_IN: &[(&str, &str)] = &[(
    "engraving",
    include_str!("../../../../extensions/apps/engraving/contract.json"),
)];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::approval;
    use ring::signature::KeyPair;
    use serde_json::json;
    use veyndo_app_model::distribution::Distribution;

    fn engraving() -> (Manifest, Contract) {
        let m: Manifest = serde_json::from_str(approval::built_in("engraving").unwrap()).unwrap();
        let c: Contract = serde_json::from_str(built_in("engraving").unwrap()).unwrap();
        (m, c)
    }

    #[test]
    fn the_bundled_contract_matches_its_manifest() {
        let (m, c) = engraving();
        validate(&m, &c, &json!({})).unwrap();
    }

    #[test]
    fn undeclared_writes_need_approval_and_declarations_cannot_hide_a_handler_effect() {
        let (m, c) = engraving();
        let save = m.actions.iter().find(|a| a.handler == "save").unwrap();
        let (undeclared, declared) = facts(&m, save, None);
        assert!(!declared && undeclared.writes && undeclared.requires_approval());
        assert_eq!(
            (undeclared.risk, undeclared.reversibility.clone()),
            (Risk::High, Reversibility::Irreversible)
        );
        let (bundled, declared) = facts(&m, save, Some(&c));
        assert!(declared && bundled.writes && !bundled.requires_approval());
        assert_eq!(bundled.permissions, vec!["catalog".to_string()]);
        assert!(bundled.data.entities.contains(&Entity::Cart));
        let mut service = save.clone();
        service.handler = "service".into();
        service.read_only = false;
        let mut lying = c.clone();
        lying
            .tools
            .insert(service.name.clone(), c.tools[&save.name].clone());
        let (f, _) = facts(&m, &service, Some(&lying));
        assert!(
            f.effects.contains(&SideEffect::ExternalCall),
            "a service call stays external"
        );
        let list = m.actions.iter().find(|a| a.handler == "list").unwrap();
        let (read, _) = facts(&m, list, None);
        assert!(!read.writes && !read.requires_approval() && read.risk == Risk::Low);
        let mut emails = c.clone();
        emails.tools.get_mut(&list.name).unwrap().effects = vec![SideEffect::Email];
        assert!(
            validate(&m, &emails, &json!({})).is_err(),
            "a read cannot send email"
        );
    }

    #[test]
    fn ports_must_match_their_action_schemas_and_versions_must_be_semantic() {
        let (m, mut c) = engraving();
        let list = m
            .actions
            .iter()
            .find(|a| a.handler == "list")
            .unwrap()
            .clone();
        c.provides = vec![ProvidedPort {
            name: "acme.rules".into(),
            version: "1.0.0".into(),
            action: list.name.clone(),
            input_schema: list.input_schema.clone(),
            output_schema: json!({"type":"object"}),
        }];
        assert!(
            validate(&m, &c, &json!({})).is_err(),
            "action declares no matching output"
        );
        c.provides.clear();
        c.consumes = vec![ConsumedPort {
            name: "Bad Name".into(),
            version: "^1".into(),
        }];
        assert!(validate(&m, &c, &json!({})).is_err());
        c.consumes = vec![ConsumedPort {
            name: "acme.rules".into(),
            version: "not-a-range".into(),
        }];
        assert!(validate(&m, &c, &json!({})).is_err());
        c.consumes.clear();
        c.rollbacks = vec![Rollback {
            to_version: m.version.clone(),
            steps: vec![],
        }];
        assert!(
            validate(&m, &c, &json!({})).is_err(),
            "a rollback must go back"
        );
        c.rollbacks.clear();
        c.version = "2".into();
        assert!(validate(&m, &c, &json!({})).is_err());
    }

    #[test]
    fn a_signed_contract_is_bound_to_its_manifest_and_tampering_is_refused() {
        let (mut m, mut c) = engraving();
        let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[9u8; 32]).unwrap();
        let keys = json!({"fixture":{"k1":STANDARD.encode(pair.public_key().as_ref())}});
        m.id = "fixture_engraving".into();
        c.app = m.id.clone();
        m.distribution = Some(Distribution {
            publisher: "fixture".into(),
            key_id: "k1".into(),
            signature: String::new(),
            channel: "stable".into(),
            dependencies: vec![],
        });
        assert!(
            validate(&m, &c, &keys).is_err(),
            "unsigned contract of a signed package"
        );
        c.signature = Some(STANDARD.encode(pair.sign(&message(&m, &c)).as_ref()));
        validate(&m, &c, &keys).unwrap();
        let mut tampered = c.clone();
        tampered.tools.values_mut().for_each(|t| t.risk = Risk::Low);
        tampered
            .tools
            .values_mut()
            .for_each(|t| t.reversibility = Reversibility::Irreversible);
        assert_eq!(
            validate(&m, &tampered, &keys).unwrap_err().0,
            StatusCode::FORBIDDEN
        );
        let mut other = m.clone();
        other.permissions.push("customers.pii".into());
        assert_eq!(
            validate(&other, &c, &keys).unwrap_err().0,
            StatusCode::FORBIDDEN
        );
        assert!(
            validate(&m, &c, &json!({})).is_err(),
            "unknown publisher key"
        );
    }
}
