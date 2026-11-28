//! Planner-facing reads of the operations graph: list admitted operations with typed facts and check a plan before execution.
//!
//! Both operations only read the compiled registry and the caller's current admission; they
//! never execute a step, grant a right or record an approval. A step that needs approval
//! still goes through the existing controls: a person applies a proposal or calls the native
//! operation, and budgeted agent credentials remain read-only.
use super::registry::{self, Effect, Operation, OperationPlan};
use super::surfaces;
use crate::apps;
use crate::foundation::App;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::HashMap;
use veyndo_app_platform::domain::app_facts::AppFacts;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::operation_facts::{Cost, OperationFacts};
use veyndo_core::request_context::RequestContext;
use veyndo_marketing::domain::flow_actions;

pub(super) const LIST: &str = "merchant.operations.list";
pub(super) const CHECK: &str = "merchant.operations.check";
/// A plan longer than this is split by the planner before it is checked.
const MAX_STEPS: usize = 50;

pub(super) const CAPABILITIES: &[(&str, &str)] = &[
    (
        LIST,
        "List the operations you may call now with typed permission, cost, reversibility, risk, data, side effects and preconditions",
    ),
    (
        CHECK,
        "Check a planned sequence of operation calls before execution: admission, arguments, prerequisites and required approvals; runs nothing",
    ),
];

pub(super) fn schema(name: &str) -> Option<Value> {
    match name {
        LIST => Some(json!({"type":"object","properties":{
            "names":{"type":"array","items":{"type":"string","maxLength":120},"maxItems":300},
            "includeUnavailable":{"type":"boolean"}
        },"additionalProperties":false})),
        CHECK => Some(json!({"type":"object","properties":{
            "steps":{"type":"array","minItems":1,"maxItems":MAX_STEPS,"items":{"type":"object","properties":{
                "name":{"type":"string","minLength":1,"maxLength":120},
                "arguments":{"type":"object"}
            },"required":["name"],"additionalProperties":false}}
        },"required":["steps"],"additionalProperties":false})),
        _ => None,
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ListRequest {
    names: Option<Vec<String>>,
    #[serde(deserialize_with = "lenient")]
    include_unavailable: Option<bool>,
}

/// One listed operation, whether the caller is admitted to it now and the UCP capabilities it implements.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Listed<'a> {
    #[serde(flatten)]
    plan: OperationPlan<'a>,
    admitted: bool,
    ucp: Vec<&'static str>,
}
/// One built-in Flow action with the facts its admission uses.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FlowAction {
    name: &'static str,
    cost: Cost,
    requires_approval: bool,
    facts: &'static OperationFacts,
    admitted: bool,
}
/// One advertised UCP capability and the operations behind it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UcpCapability {
    id: &'static str,
    version: &'static str,
    operations: &'static [&'static str],
    requires_approval: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CheckRequest {
    steps: Vec<Step>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Step {
    name: String,
    arguments: Option<Value>,
}

/// What the planner may do with a step.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum Verdict {
    /// The caller may run the step directly.
    Allowed,
    /// A person must approve the step through the existing proposal or native operation path.
    Approval,
    /// The caller cannot run the step now.
    Blocked,
    /// The name is no native operation of this registry.
    Unknown,
}

/// Why a step received its verdict; stable codes for planners and Studio vocabularies.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum Reason {
    NotAuthorized,
    AgentCredentialReadOnly,
    InvalidArguments,
    HighRisk,
    Irreversible,
    UndeclaredFacts,
    SandboxOnly,
    PersonalSession,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Checked<'a> {
    index: usize,
    name: &'a str,
    verdict: Verdict,
    reasons: Vec<Reason>,
    /// Prerequisite operations this plan does not run before the step; their result must already exist.
    unmet_requires: Vec<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compensation: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    argument_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanCheck<'a> {
    steps: Vec<Checked<'a>>,
    /// At least one step needs a person's approval.
    approval_required: bool,
    /// No step is blocked or unknown.
    executable: bool,
}

/// Facts of each installed app tool named by a plan, loaded once per distinct step; `None` means no such tool.
type AppStepFacts = HashMap<String, Option<(AppFacts, bool)>>;

pub(super) async fn invoke(a: &App, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    match name {
        LIST => Ok(list(h, view(v))),
        CHECK => {
            let request: CheckRequest = view(v);
            if request.steps.is_empty() || request.steps.len() > MAX_STEPS {
                return Err(bad("A plan has between 1 and 50 steps"));
            }
            let mut apps = AppStepFacts::new();
            for step in request
                .steps
                .iter()
                .filter(|step| step.name.starts_with("app."))
            {
                if !apps.contains_key(&step.name) {
                    let facts =
                        veyndo_app_platform::store::tool_catalogue::tool_facts(a, h, &step.name)
                            .await?;
                    apps.insert(step.name.clone(), facts);
                }
            }
            Ok(json!(check(h, &request.steps, &apps)))
        }
        _ => Err(bad("Unknown planner operation")),
    }
}

fn list(h: &RequestContext, request: ListRequest) -> Value {
    let include = request.include_unavailable.unwrap_or(false);
    let operations = registry::OPERATIONS
        .iter()
        .filter(|operation| {
            request
                .names
                .as_ref()
                .is_none_or(|names| names.iter().any(|name| name == operation.name))
        })
        .map(|operation| Listed {
            plan: operation.plan(),
            admitted: admitted(operation, h),
            ucp: surfaces::UCP
                .iter()
                .filter(|declaration| declaration.operations.contains(&operation.name))
                .map(|declaration| declaration.id)
                .collect(),
        })
        .filter(|listed| include || listed.admitted)
        .collect::<Vec<_>>();
    let flow_actions = flow_actions::ACTIONS
        .iter()
        .map(|(name, facts)| FlowAction {
            name,
            cost: facts.cost(),
            requires_approval: facts.requires_approval(),
            facts,
            admitted: veyndo_identity::merchant(h).is_ok()
                && facts
                    .permissions
                    .iter()
                    .all(|permission| veyndo_identity::allowed(h, permission)),
        })
        .filter(|action| include || action.admitted)
        .collect::<Vec<_>>();
    let ucp = surfaces::UCP
        .iter()
        .map(|declaration| UcpCapability {
            id: declaration.id,
            version: declaration.version,
            operations: declaration.operations,
            requires_approval: declaration.operations.iter().any(|name| {
                registry::find(name).is_some_and(|operation| operation.facts.requires_approval())
            }),
        })
        .collect::<Vec<_>>();
    json!({"version":1,"operations":operations,"flowActions":flow_actions,"ucpCapabilities":ucp})
}

/// The same admission the dispatcher applies; agent credentials remain limited to reads.
fn admitted(operation: &Operation, h: &RequestContext) -> bool {
    operation.admitted_now(h)
}

fn check<'a>(h: &RequestContext, steps: &'a [Step], apps: &'a AppStepFacts) -> PlanCheck<'a> {
    let mut checked = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        if step.name.starts_with("app.") {
            checked.push(app_step(
                h,
                steps,
                index,
                apps.get(&step.name).and_then(Option::as_ref),
            ));
            continue;
        }
        let Some(operation) = registry::find(&step.name) else {
            checked.push(Checked {
                index,
                name: &step.name,
                verdict: Verdict::Unknown,
                reasons: vec![],
                unmet_requires: vec![],
                compensation: None,
                argument_error: None,
            });
            continue;
        };
        let facts = &operation.facts;
        let mut reasons = vec![];
        let agent = h.principal.agent_key.is_some();
        if !admitted(operation, h) {
            reasons.push(
                if agent && operation.effect != Effect::Read && admitted_as_person(operation, h) {
                    Reason::AgentCredentialReadOnly
                } else {
                    Reason::NotAuthorized
                },
            );
        }
        let argument_error = step.arguments.as_ref().and_then(|arguments| {
            apps::validate_native_input(&operation.input_schema(), arguments)
                .err()
                .map(|error| error.1)
        });
        if argument_error.is_some() {
            reasons.push(Reason::InvalidArguments);
        }
        let blocked = !reasons.is_empty();
        if facts.writes && facts.risk == veyndo_core::operation_facts::Risk::High {
            reasons.push(Reason::HighRisk);
        }
        if facts.writes
            && facts.reversibility == veyndo_core::operation_facts::Reversibility::Irreversible
        {
            reasons.push(Reason::Irreversible);
        }
        for condition in facts.conditions {
            reasons.push(match condition {
                veyndo_core::operation_facts::Condition::Sandbox => Reason::SandboxOnly,
                veyndo_core::operation_facts::Condition::PersonalSession => Reason::PersonalSession,
            });
        }
        let earlier = &steps[..index];
        let unmet_requires = facts
            .requires
            .iter()
            .copied()
            .filter(|required| !earlier.iter().any(|step| step.name == *required))
            .map(Cow::Borrowed)
            .collect();
        checked.push(Checked {
            index,
            name: &step.name,
            verdict: if blocked {
                Verdict::Blocked
            } else if facts.requires_approval() {
                Verdict::Approval
            } else {
                Verdict::Allowed
            },
            reasons,
            unmet_requires,
            compensation: facts.compensation().map(Cow::Borrowed),
            argument_error,
        });
    }
    PlanCheck {
        approval_required: checked.iter().any(|step| step.verdict == Verdict::Approval),
        executable: checked
            .iter()
            .all(|step| matches!(step.verdict, Verdict::Allowed | Verdict::Approval)),
        steps: checked,
    }
}

/// An installed app tool step gets the same reasons and verdicts as a native operation from its manifest facts;
/// facts the publisher did not declare are conservative defaults and additionally marked `undeclaredFacts`.
fn app_step<'a>(
    h: &RequestContext,
    steps: &'a [Step],
    index: usize,
    found: Option<&'a (AppFacts, bool)>,
) -> Checked<'a> {
    use veyndo_app_platform::domain::app_facts::{Audience, Condition, Reversibility, Risk};
    let step = &steps[index];
    let Some((facts, declared)) = found else {
        return Checked {
            index,
            name: &step.name,
            verdict: Verdict::Unknown,
            reasons: vec![],
            unmet_requires: vec![],
            compensation: None,
            argument_error: None,
        };
    };
    let mut reasons = vec![];
    let permitted = facts
        .permissions
        .iter()
        .all(|permission| veyndo_identity::allowed(h, permission));
    let person =
        facts.audience == Audience::Shopper || (veyndo_identity::merchant(h).is_ok() && permitted);
    if !person {
        reasons.push(Reason::NotAuthorized);
    } else if facts.writes && h.principal.agent_key.is_some() {
        reasons.push(Reason::AgentCredentialReadOnly);
    }
    let blocked = !reasons.is_empty();
    if facts.writes && facts.risk == Risk::High {
        reasons.push(Reason::HighRisk);
    }
    if facts.writes && facts.reversibility == Reversibility::Irreversible {
        reasons.push(Reason::Irreversible);
    }
    for condition in &facts.conditions {
        reasons.push(match condition {
            Condition::Sandbox => Reason::SandboxOnly,
            Condition::PersonalSession => Reason::PersonalSession,
        });
    }
    if !declared {
        reasons.push(Reason::UndeclaredFacts);
    }
    let earlier = &steps[..index];
    Checked {
        index,
        name: &step.name,
        verdict: if blocked {
            Verdict::Blocked
        } else if facts.requires_approval() || !declared {
            Verdict::Approval
        } else {
            Verdict::Allowed
        },
        reasons,
        unmet_requires: facts
            .requires
            .iter()
            .filter(|required| !earlier.iter().any(|step| &step.name == *required))
            .map(|required| Cow::Borrowed(required.as_str()))
            .collect(),
        compensation: None,
        argument_error: None,
    }
}

/// Whether the same principal without its agent credential would be admitted: the credential, not the role, blocks.
fn admitted_as_person(operation: &Operation, h: &RequestContext) -> bool {
    let mut person = h.clone();
    person.principal.agent_key = None;
    operation.admitted_now(&person)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merchant(permissions: &[&str]) -> RequestContext {
        let mut h = RequestContext::new();
        for (key, value) in [
            ("x-rac-role", "member".to_owned()),
            ("x-rac-tenant", "demo".to_owned()),
            ("x-tenant", "demo".to_owned()),
            ("x-rac-user", "u1".to_owned()),
            (
                "x-rac-permissions",
                serde_json::to_string(permissions).unwrap(),
            ),
        ] {
            h.insert(key, value.parse().unwrap());
        }
        h
    }
    fn steps(names: &[(&str, Option<Value>)]) -> Vec<Step> {
        names
            .iter()
            .map(|(name, arguments)| Step {
                name: (*name).into(),
                arguments: arguments.clone(),
            })
            .collect()
    }

    #[test]
    fn listing_returns_only_admitted_operations_with_their_facts() {
        let h = merchant(&["catalog.read", "catalog.write"]);
        let listed = list(&h, ListRequest::default());
        let names = listed["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.contains(&"merchant.product.save"));
        assert!(names.contains(&LIST));
        assert!(!names.contains(&"merchant.payment"));
        let save = &listed["operations"][names
            .iter()
            .position(|n| *n == "merchant.product.save")
            .unwrap()];
        assert_eq!(save["admitted"], true);
        assert_eq!(save["facts"]["permissions"], json!(["catalog.write"]));
        assert_eq!(
            save["facts"]["reversibility"],
            json!({"kind":"compensatable","by":"merchant.history.restore"})
        );
        assert_eq!(save["preconditions"]["expectedRevision"], false);
        let all = list(
            &h,
            ListRequest {
                names: Some(vec!["merchant.payment".into()]),
                include_unavailable: Some(true),
            },
        );
        assert_eq!(all["operations"][0]["admitted"], false);
        assert_eq!(all["operations"][0]["requiresApproval"], true);
        assert_eq!(all["operations"][0]["cost"], "provider");
        let flow = listed["flowActions"].as_array().unwrap();
        assert!(flow.iter().all(|action| action["admitted"] == true));
        assert!(
            !flow
                .iter()
                .any(|action| action["name"] == "action.mail.send")
        );
        let checkout = list(
            &RequestContext::new(),
            ListRequest {
                names: Some(vec!["checkout.complete".into()]),
                include_unavailable: Some(true),
            },
        );
        assert_eq!(
            checkout["operations"][0]["ucp"],
            json!(["dev.ucp.shopping.checkout"])
        );
        assert_eq!(
            checkout["flowActions"].as_array().unwrap().len(),
            flow_actions::ACTIONS.len()
        );
        assert!(
            checkout["ucpCapabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["requiresApproval"] == true)
        );
    }

    #[test]
    fn plan_check_separates_allowed_approval_blocked_and_unknown_steps() {
        let h = merchant(&[
            "catalog.read",
            "catalog.write",
            "assistant.use",
            "orders.read",
            "knowledge.read",
        ]);
        let plan = steps(&[
            ("merchant.products", Some(json!({"limit":10}))),
            ("merchant.apply", None),
            ("merchant.payment", None),
            ("merchant.products", Some(json!({"limit":"ten"}))),
            ("app.demo.sync", None),
            ("no.such.operation", None),
        ]);
        let undeclared = AppFacts {
            audience: veyndo_app_platform::domain::app_facts::Audience::Merchant,
            permissions: vec![],
            writes: true,
            reversibility: veyndo_app_platform::domain::app_facts::Reversibility::Irreversible,
            risk: veyndo_app_platform::domain::app_facts::Risk::High,
            data: Default::default(),
            effects: vec![],
            requires: vec![],
            conditions: vec![],
        };
        let mut apps = AppStepFacts::new();
        apps.insert("app.demo.sync".into(), Some((undeclared, false)));
        let result = json!(check(&h, &plan, &apps));
        assert_eq!(
            result["steps"][4]["reasons"],
            json!(["highRisk", "irreversible", "undeclaredFacts"])
        );
        let verdicts = result["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["verdict"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            verdicts,
            [
                "allowed", "approval", "blocked", "blocked", "approval", "unknown"
            ]
        );
        assert_eq!(result["steps"][1]["reasons"], json!(["highRisk"]));
        assert_eq!(
            result["steps"][1]["unmetRequires"],
            json!(["merchant.plan"])
        );
        assert_eq!(
            result["steps"][1]["compensation"],
            "merchant.history.restore"
        );
        assert_eq!(result["steps"][2]["reasons"][0], "notAuthorized");
        assert_eq!(result["steps"][3]["reasons"], json!(["invalidArguments"]));
        assert!(result["steps"][3]["argumentError"].is_string());
        assert_eq!(result["approvalRequired"], true);
        assert_eq!(result["executable"], false);
        let ordered = json!(check(
            &h,
            &steps(&[("merchant.plan", None), ("merchant.apply", None)]),
            &AppStepFacts::new()
        ));
        assert_eq!(ordered["steps"][1]["unmetRequires"], json!([]));
        assert_eq!(ordered["executable"], true);
    }

    #[test]
    fn agent_credentials_see_writes_as_blocked_by_the_credential() {
        let mut h = merchant(&["catalog.read", "catalog.write"]);
        h.principal.agent_key = Some("agent".into());
        let result = json!(check(
            &h,
            &steps(&[("merchant.product.save", None), ("merchant.products", None)]),
            &AppStepFacts::new()
        ));
        assert_eq!(result["steps"][0]["verdict"], "blocked");
        assert_eq!(result["steps"][0]["reasons"][0], "agentCredentialReadOnly");
        assert_eq!(result["steps"][1]["verdict"], "allowed");
    }
}
