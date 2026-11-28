//! Explanations of agent actions: each tool call, proposal, twin simulation and applied change records a rationale and the references it relied on. Sources are IDs, revisions and digests, never copied results, credentials or customer data; model text stays marked untrusted.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::hash;

/// The explanation document version stored with a proposal.
pub const VERSION: u32 = 1;
/// At most this many sources are recorded; the rest are counted.
const SOURCE_LIMIT: usize = 120;

/// A rationale and its sources for every action of one agent run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub version: u32,
    pub actions: Vec<Action>,
    pub sources: Vec<Source>,
    /// Sources beyond the stored limit.
    #[serde(default)]
    pub omitted_sources: usize,
}

/// One agent action and why it happened.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    /// `toolCall`, `proposal`, `simulation` or `applied`.
    pub kind: String,
    /// The native operation of a tool call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub rationale: Rationale,
    /// The IDs of the sources this action relied on.
    pub source_ids: Vec<String>,
}

/// Why an action happened: model text (untrusted) or a stable code of the system that decided.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rationale {
    /// `model`, `planner`, `twin`, `merchant` or `policy`.
    pub origin: String,
    /// Model-written text is untrusted data, shown as such and never executed or followed.
    pub untrusted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// A reference to data, a rule, an event or model output an action relied on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    /// `instruction`, `product`, `operation`, `document`, `externalSource`, `knowledge`, `facts`, `rule`, `orders`, `app`, `event`, `model` or `history`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The referenced record: an ID or a stable name, never its content.
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    /// A SHA-256 digest of the exact data used, to verify it without storing it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// The permission a reader needs to see this reference.
    pub scope: String,
}

/// The run evidence the proposal explanation reads.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RunEvidence {
    pub instruction: Value,
    pub model: Value,
    pub inference: Value,
    pub proposal: Value,
    pub catalog_before: Vec<Value>,
    pub tool_trace: Vec<Value>,
    pub document_sources: Value,
    pub external_sources: Value,
    pub knowledge: Value,
    pub verified_facts: Value,
    pub app_actions: Value,
    #[serde(deserialize_with = "lenient")]
    pub policy_revision: Option<i64>,
    pub proposal_repair: Value,
    pub simulation: Value,
    /// The prompt of a proposal that did not come from the merchant planner.
    #[serde(deserialize_with = "lenient")]
    pub prompt_id: Option<String>,
}

/// Identity fields of a referenced record, as stored.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Reference {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    content_hash: Option<String>,
    #[serde(deserialize_with = "lenient")]
    digest: Option<String>,
    #[serde(deserialize_with = "lenient")]
    app: Option<String>,
    #[serde(deserialize_with = "lenient")]
    revision: Option<i64>,
}

/// One executed read of the tool trace.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Read {
    #[serde(deserialize_with = "lenient")]
    tool: Option<String>,
    input: Value,
    result: Value,
}

/// The model-written parts of a proposal.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Written {
    #[serde(deserialize_with = "lenient")]
    summary: Option<String>,
    #[serde(deserialize_with = "lenient")]
    rationale: Option<String>,
    changes: Vec<Value>,
}

/// The simulation fields an explanation references.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Simulated {
    #[serde(deserialize_with = "lenient")]
    status: Option<String>,
    rules: Vec<Value>,
    impact: Value,
    #[serde(deserialize_with = "lenient")]
    simulated_at: Option<String>,
}

/// One rule the simulation evaluated.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EvaluatedRule {
    #[serde(deserialize_with = "lenient")]
    rule: Option<String>,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
}

/// The digest of a JSON value's canonical text.
fn digest(v: &Value) -> String {
    hash(&v.to_string())
}

/// Collects sources, deduplicated by type and reference, under the storage limit.
struct Sources {
    list: Vec<Source>,
    omitted: usize,
}

impl Sources {
    fn add(
        &mut self,
        kind: &str,
        reference: &str,
        revision: Option<i64>,
        digest: Option<String>,
        scope: &str,
    ) -> Option<String> {
        if let Some(found) = self
            .list
            .iter()
            .find(|s| s.kind == kind && s.reference == reference && s.revision == revision)
        {
            return Some(found.id.clone());
        }
        if self.list.len() >= SOURCE_LIMIT {
            self.omitted += 1;
            return None;
        }
        let id = format!("s{}", self.list.len() + 1);
        self.list.push(Source {
            id: id.clone(),
            kind: kind.into(),
            reference: reference.into(),
            revision,
            digest,
            scope: scope.into(),
        });
        Some(id)
    }
}

/// The prompt that produced the final proposal.
fn prompt_id(e: &RunEvidence) -> &str {
    if let Some(prompt) = &e.prompt_id {
        prompt
    } else if e.proposal_repair.is_null() {
        "merchant-planner@6"
    } else {
        "merchant-repair@1"
    }
}

/// The explanation of a planned proposal: its read tool calls, the proposal and, when present, the twin simulation.
/// `event` names the outbox event or Flow job that started the run, if any.
pub fn proposal(e: &RunEvidence, event: Option<&str>) -> Explanation {
    let mut sources = Sources {
        list: vec![],
        omitted: 0,
    };
    let mut actions = vec![];
    let instruction = sources.add(
        "instruction",
        "merchant",
        None,
        Some(digest(&e.instruction)),
        "assistant.use",
    );
    let started = event.and_then(|event| sources.add("event", event, None, None, "assistant.use"));
    for (round, read) in e.tool_trace.iter().map(view::<Read>).enumerate() {
        let tool = read.tool.unwrap_or_default();
        let call = sources.add(
            "operation",
            &format!("{tool}#{}", round + 1),
            None,
            Some(digest(&json!({"input":read.input,"result":read.result}))),
            "assistant.use",
        );
        actions.push(Action {
            kind: "toolCall".into(),
            tool: Some(tool),
            at: None,
            actor: None,
            rationale: Rationale {
                origin: "planner".into(),
                untrusted: false,
                text: None,
                code: Some("readRequestedByModel".into()),
            },
            source_ids: instruction.iter().chain(call.iter()).cloned().collect(),
        });
    }
    let mut used: Vec<String> = instruction.iter().chain(started.iter()).cloned().collect();
    let written = view::<Written>(&e.proposal);
    let changed = written
        .changes
        .iter()
        .filter_map(|c| c.get("product_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if !e.knowledge.is_null() {
        used.extend(sources.add(
            "knowledge",
            "productNeighborhood",
            None,
            Some(digest(&e.knowledge)),
            "knowledge.read",
        ));
    }
    if !e.verified_facts.is_null() {
        used.extend(sources.add(
            "facts",
            "shopObservations",
            None,
            Some(digest(&e.verified_facts)),
            "assistant.use",
        ));
    }
    if let Some(revision) = e.policy_revision {
        used.extend(sources.add("rule", "aiPolicy", Some(revision), None, "assistant.use"));
    }
    let model = format!(
        "{}/{}",
        e.inference.as_str().unwrap_or("platform"),
        e.model.as_str().unwrap_or("default")
    );
    used.extend(sources.add(
        "model",
        &format!("{model}@{}", prompt_id(e)),
        None,
        Some(digest(&e.proposal)),
        "assistant.use",
    ));
    for product in &e.catalog_before {
        let r = view::<Reference>(product);
        let Some(id) = r.id else { continue };
        // Products come last: a large catalog context may be counted as omitted, never the model or rule sources.
        // Every product the model saw is context; changed products are also the changed records.
        if let Some(s) = sources.add("product", &id, r.revision, None, "catalog.read")
            && changed.contains(&id.as_str())
        {
            used.push(s);
        }
    }
    for document in e.document_sources.as_array().into_iter().flatten() {
        let r = view::<Reference>(document);
        if let Some(id) = r.source_id {
            used.extend(sources.add(
                "document",
                &id,
                r.revision,
                r.content_hash,
                "knowledge.read",
            ));
        }
    }
    for external in e.external_sources.as_array().into_iter().flatten() {
        let r = view::<Reference>(external);
        if let Some(id) = r.source_id.or(r.id) {
            let reference = format!("{}/{id}", r.app.unwrap_or_default());
            used.extend(sources.add("externalSource", &reference, None, r.digest, "apps.read"));
        }
    }
    for app in e.app_actions.as_array().into_iter().flatten() {
        if let Some(id) = view::<Reference>(app).id {
            used.extend(sources.add("app", &id, None, None, "apps.read"));
        }
    }
    let text = written
        .rationale
        .filter(|r| !r.trim().is_empty())
        .or(written.summary)
        .map(|t| t.chars().take(2000).collect());
    actions.push(Action {
        kind: "proposal".into(),
        tool: None,
        at: None,
        actor: None,
        rationale: Rationale {
            origin: "model".into(),
            untrusted: true,
            text,
            code: None,
        },
        source_ids: used,
    });
    if !e.simulation.is_null() {
        actions.push(simulation(&mut sources, &e.simulation));
    }
    Explanation {
        version: VERSION,
        actions,
        sources: sources.list,
        omitted_sources: sources.omitted,
    }
}

/// The twin's action: the rules it evaluated and the recorded order window it projected from.
fn simulation(sources: &mut Sources, s: &Value) -> Action {
    let simulated = view::<Simulated>(s);
    let mut used = vec![];
    for rule in simulated.rules.iter().map(view::<EvaluatedRule>) {
        if let (Some(rule), Some(product)) = (rule.rule, rule.product_id) {
            used.extend(sources.add(
                "rule",
                &format!("{rule}:{product}"),
                None,
                None,
                "assistant.use",
            ));
        }
    }
    if simulated.impact.is_object() {
        let window = simulated
            .impact
            .get("windowDays")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        used.extend(sources.add(
            "orders",
            &format!("trailing{window}Days"),
            None,
            Some(digest(&simulated.impact)),
            "orders.read",
        ));
    }
    Action {
        kind: "simulation".into(),
        tool: None,
        at: simulated.simulated_at,
        actor: None,
        rationale: Rationale {
            origin: "twin".into(),
            untrusted: false,
            text: None,
            code: simulated.status,
        },
        source_ids: used,
    }
}

/// The applied action: who approved under which mode, with references to the simulation, the policy revision and the history entry.
pub fn applied(
    task: &str,
    actor: Option<&str>,
    autonomous: bool,
    policy_revision: i64,
    simulation: &Value,
    at: &str,
) -> Value {
    let mut sources = vec![
        json!({"type":"rule","reference":"aiPolicy","revision":policy_revision,"scope":"assistant.use"}),
        json!({"type":"history","reference":history_reason(task),"scope":"catalog.read"}),
        json!({"type":"event","reference":"merchant.change.applied","scope":"assistant.use"}),
    ];
    if !simulation.is_null() {
        sources.push(json!({"type":"simulation","reference":task,"digest":digest(simulation),"scope":"assistant.use"}));
    }
    json!({
        "kind":"applied","at":at,"actor":actor,
        "rationale":{"origin":if autonomous {"policy"} else {"merchant"},"untrusted":false,
            "code":if autonomous {"withinAutonomyPolicy"} else {"approvedAfterReview"}},
        "sourceIds":[],"sources":sources
    })
}

/// The history reason recorded with every change an applied proposal writes.
pub fn history_reason(task: &str) -> String {
    format!("agent-proposal:{task}")
}

/// The explanation a reader may see: sources needing a permission they lack are removed, as are their IDs on actions.
pub fn visible(explanation: &Value, allowed: impl Fn(&str) -> bool) -> Value {
    let mut e = explanation.clone();
    let mut hidden = vec![];
    if let Some(list) = e.get_mut("sources").and_then(Value::as_array_mut) {
        list.retain(|s| {
            let keep = s.get("scope").and_then(Value::as_str).is_some_and(&allowed);
            if !keep && let Some(id) = s.get("id").and_then(Value::as_str) {
                hidden.push(id.to_string());
            }
            keep
        });
    }
    for action in e
        .get_mut("actions")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(ids) = action.get_mut("sourceIds").and_then(Value::as_array_mut) {
            ids.retain(|id| id.as_str().is_none_or(|id| !hidden.iter().any(|h| h == id)));
        }
        if let Some(inline) = action.get_mut("sources").and_then(Value::as_array_mut) {
            inline.retain(|s| s.get("scope").and_then(Value::as_str).is_some_and(&allowed));
        }
    }
    if !hidden.is_empty() {
        let _ = veyndo_core::document::merge(&mut e, json!({ "hiddenSources": hidden.len() }));
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> RunEvidence {
        view(&json!({
            "instruction":"Lower the lamp price",
            "model":"gpt-x","inference":"openai",
            "proposal":{"summary":"Lower lamp","rationale":"Sales slowed; corridor allows it","changes":[{"product_id":"lamp","price":72.5}]},
            "catalogBefore":[{"id":"lamp","revision":4},{"id":"chair","revision":2}],
            "toolTrace":[{"tool":"merchant.orders","input":{},"result":{"orders":[{"email":"buyer@example.test"}]}}],
            "documentSources":[{"sourceId":"doc-1","contentHash":"abc","revision":3}],
            "externalSources":[],
            "knowledge":{"pairs":[]},
            "verifiedFacts":{"orderCount":3},
            "appActions":[],
            "policyRevision":7,
            "proposalRepair":null,
            "simulation":{"status":"simulated","simulatedAt":"2026-11-27T10:00:00Z",
                "rules":[{"rule":"priceGuardrail","productId":"lamp"}],"impact":{"windowDays":30}}
        }))
    }

    #[test]
    fn a_proposal_explanation_references_its_sources_without_copying_them() {
        let e = proposal(&evidence(), Some("flow-job:j1"));
        let kinds = e
            .actions
            .iter()
            .map(|a| a.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["toolCall", "proposal", "simulation"]);
        let text = serde_json::to_string(&e).unwrap();
        assert!(
            !text.contains("buyer@example.test"),
            "tool results are digested, never copied"
        );
        assert!(
            !text.contains("Lower the lamp price"),
            "the instruction is referenced by digest"
        );
        let proposal = &e.actions[1];
        assert!(proposal.rationale.untrusted);
        assert_eq!(
            proposal.rationale.text.as_deref(),
            Some("Sales slowed; corridor allows it")
        );
        let source = |id: &str| e.sources.iter().find(|s| s.id == id).unwrap();
        let used = proposal
            .source_ids
            .iter()
            .map(|id| source(id))
            .collect::<Vec<_>>();
        assert!(
            used.iter()
                .any(|s| s.kind == "product" && s.reference == "lamp" && s.revision == Some(4))
        );
        assert!(
            !used.iter().any(|s| s.reference == "chair"),
            "unchanged context products are not cited as changed records"
        );
        assert!(
            used.iter()
                .any(|s| s.kind == "document" && s.digest.as_deref() == Some("abc"))
        );
        assert!(
            used.iter()
                .any(|s| s.kind == "rule" && s.revision == Some(7))
        );
        assert!(
            used.iter()
                .any(|s| s.kind == "event" && s.reference == "flow-job:j1")
        );
        assert!(
            used.iter()
                .any(|s| s.kind == "model" && s.reference == "openai/gpt-x@merchant-planner@6")
        );
        let simulation = &e.actions[2];
        assert_eq!(simulation.rationale.code.as_deref(), Some("simulated"));
        assert!(
            simulation
                .source_ids
                .iter()
                .any(|id| source(id).scope == "orders.read")
        );
    }

    #[test]
    fn readers_see_only_sources_their_permissions_cover() {
        let full = json!(proposal(&evidence(), None));
        let limited = visible(&full, |scope| scope == "assistant.use");
        let sources = limited["sources"].as_array().unwrap();
        assert!(sources.iter().all(|s| s["scope"] == "assistant.use"));
        assert!(limited["hiddenSources"].as_u64().unwrap() > 0);
        let kept = sources.iter().map(|s| s["id"].clone()).collect::<Vec<_>>();
        for action in limited["actions"].as_array().unwrap() {
            for id in action["sourceIds"].as_array().unwrap() {
                assert!(kept.contains(id));
            }
        }
        assert_eq!(visible(&full, |_| true), full);
    }

    #[test]
    fn the_applied_action_names_its_mode_history_and_simulation() {
        let action = applied(
            "t1",
            Some("u1"),
            false,
            7,
            &json!({"status":"simulated"}),
            "now",
        );
        assert_eq!(action["rationale"]["code"], "approvedAfterReview");
        assert!(
            action["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["reference"] == "agent-proposal:t1")
        );
        let autonomous = applied("t1", None, true, 7, &Value::Null, "now");
        assert_eq!(autonomous["rationale"]["origin"], "policy");
        assert_eq!(autonomous["sources"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn sources_are_bounded_and_counted() {
        let mut e = evidence();
        e.catalog_before = (0..200)
            .map(|i| json!({"id":format!("p{i}"),"revision":1}))
            .collect();
        let explained = proposal(&e, None);
        assert_eq!(explained.sources.len(), SOURCE_LIMIT);
        assert!(explained.omitted_sources > 0);
    }
}
