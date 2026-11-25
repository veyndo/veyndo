//! Fictional shopper cohorts reuse native task authority, checkpoints and private commerce operations.
use super::agent_runs::{enqueue, operate};
use crate::ports::AgentHost;
use crate::store::product_reads::{private_sandbox, scenario_runs};
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::model_contract::validate_native;
use veyndo_core::shop_scope::{channel as channel_id, header};
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad, conflict, hash};
use veyndo_identity::{allowed, assistant_access, merchant, permit};
use veyndo_shipping::domain::geography::catalogue;
mod attack_store;
mod catalogue;
mod evidence;
mod journey;
mod probes;
mod readiness;
mod shopper;
mod source;
pub use journey::execute;
/// The scenario operations.
pub const CAPABILITIES: &[(&str, &str)] = &[
    (
        "merchant.scenario.create",
        "Run a bounded fictional customer cohort in a private sandbox",
    ),
    (
        "merchant.scenario.status",
        "Read actor-private customer journey observations",
    ),
    (
        "merchant.scenario.list",
        "List the current actor's sandbox scenarios",
    ),
    (
        "merchant.scenario.readiness",
        "Read whether a private sandbox is ready to be tested as a shop twin",
    ),
    (
        "merchant.scenario.stock",
        "Give empty products of a private sandbox a test stock quantity",
    ),
    (
        "merchant.scenario.attacks",
        "Read the latest saved attack test of a private sandbox",
    ),
    (
        "merchant.scenario.probe",
        "Run scripted cart and checkout attacks against a private sandbox",
    ),
];
/// Whether the caller holds every right a customer simulation needs.
pub fn visible(h: &RequestContext) -> bool {
    h.principal.app.is_none()
        && [
            "assistant.use",
            "catalog.read",
            "channels.read",
            "environments.manage",
            "orders.write",
            "orders.read",
            "knowledge.read",
        ]
        .iter()
        .all(|s| allowed(h, s))
}
/// The input schema of a scenario operation.
pub fn schema(name: &str) -> Option<Value> {
    if !CAPABILITIES.iter().any(|(n, _)| *n == name) {
        return None;
    }
    let profile = json!({"type":"object","required":["id","name","goal","budgetMinor","locale","country"],"additionalProperties":false,"properties":{
        "id":{"type":"string","minLength":1,"maxLength":24},"name":{"type":"string","minLength":1,"maxLength":80},
        "goal":{"type":"string","minLength":1,"maxLength":800},"budgetMinor":{"type":"integer","minimum":1,"maximum":100000000},
        "locale":{"type":"string","maxLength":35},"country":{"type":"string","minLength":2,"maxLength":2}}});
    Some(match name {
        "merchant.scenario.create" => {
            json!({"type":"object","required":["requestId","title","instruction","profiles"],"additionalProperties":false,"properties":{
            "sourceTaskId":{"type":"string","minLength":1,"maxLength":64},"requestId":{"type":"string","minLength":1,"maxLength":40},"title":{"type":"string","minLength":1,"maxLength":100},
            "instruction":{"type":"string","minLength":1,"maxLength":2000},"profiles":{"type":"array","minItems":1,"maxItems":6,"items":profile},
            "inference":{"type":"object","properties":{"provider":{"type":"string","enum":["platform","ollama","openai","anthropic"]},"model":{"type":["string","null"],"maxLength":128}},"required":["provider"],"additionalProperties":false}}})
        }
        "merchant.scenario.status" => {
            json!({"type":"object","required":["id"],"additionalProperties":false,"properties":{"id":{"type":"string","minLength":1,"maxLength":40}}})
        }
        "merchant.scenario.stock" => {
            json!({"type":"object","required":["quantity","approve"],"additionalProperties":false,"properties":{"quantity":{"type":"integer","minimum":1,"maximum":1000},"approve":{"type":"boolean"}}})
        }
        _ => json!({"type":"object","properties":{},"additionalProperties":false}),
    })
}
/// Admits a scenario caller in a private sandbox and returns the sandbox.
pub async fn admit(a: &State, h: &RequestContext) -> Result<String> {
    assistant_access(h)?;
    if !visible(h) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Customer simulations require sandbox, catalogue and order permissions".into(),
        ));
    }
    let t = merchant(h)?;
    let private = private_sandbox(&a.db, &t).await?;
    if !private {
        return Err(conflict(
            "Open a private sandbox before simulating customers",
        ));
    }
    Ok(t)
}
/// The input of a scenario operation, already checked against its schema.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ScenarioOperation {
    #[serde(deserialize_with = "lenient")]
    approve: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    quantity: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    request_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    source_task_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    profiles: Vec<Value>,
    instruction: Value,
    title: Value,
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
}

/// The identifying fields of one fictional customer profile.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ProfileKeys {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    locale: Option<String>,
    #[serde(deserialize_with = "lenient")]
    country: Option<String>,
}

/// Runs a scenario operation.
pub async fn invoke<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    let a = host.models().core;
    validate_native(&schema(name).ok_or(bad("Unknown scenario operation"))?, v)?;
    let t = admit(a, h).await?;
    let input: ScenarioOperation = view(v);
    if name == "merchant.scenario.readiness" {
        return readiness::read(host, h, &t).await;
    }
    if name == "merchant.scenario.probe" {
        let mut report = probes::run(host, h, &t).await?;
        attack_store::save(a, h, &t, &report).await?;
        report["configurationReview"] = json!("current");
        return Ok(report);
    }
    if name == "merchant.scenario.stock" {
        permit(h, "catalog.write")?;
        if input.approve != Some(true) {
            return Err(bad("Explicit approve=true required"));
        }
        return host
            .fill_test_stock(&t, input.quantity.unwrap_or(0) as i32)
            .await;
    }
    if name == "merchant.scenario.attacks" {
        return Ok(json!({"latest":attack_store::latest(host, h, &t).await?}));
    }
    if name == "merchant.scenario.create" {
        let group = input
            .request_id
            .as_deref()
            .ok_or(bad("Invalid scenario request ID"))?;
        let source = match input.source_task_id.as_deref() {
            Some(id) => source::resolve(a, h, id).await?,
            None => Value::Null,
        };
        if !identifier(group) {
            return Err(bad("Invalid scenario request ID"));
        }
        let profiles = &input.profiles;
        let settings = catalogue::settings(a, h, &t).await?;
        let mut ids = std::collections::HashSet::new();
        let keys = profiles.iter().map(view::<ProfileKeys>).collect::<Vec<_>>();
        for p in &keys {
            let id = p.id.as_deref().unwrap_or("");
            if !identifier(id) || !ids.insert(id) {
                return Err(bad("Customer profile IDs must be unique identifiers"));
            }
            if !settings
                .locales
                .iter()
                .any(|l| Some(l.as_str()) == p.locale.as_deref())
                || !settings
                    .countries
                    .iter()
                    .any(|c| Some(c.as_str()) == p.country.as_deref())
            {
                return Err(bad(
                    "Customer profiles must use enabled shop languages and countries",
                ));
            }
        }
        let currency = settings.currencies.selected("")?;
        let digest = hash(&v.to_string());
        let comparison = hash(&json!({"instruction":input.instruction,"profiles":profiles,"inference":v.get("inference"),"channel":channel_id(h),"currency":currency.code,"scale":currency.scale}).to_string());
        // Each member has its own provider fence. A transport retry safely completes
        // a partially queued cohort without recharging or duplicating earlier members.
        let mut runs = Vec::new();
        for (index, (profile, keys)) in profiles.iter().zip(&keys).enumerate() {
            let request = json!({"instruction":input.instruction,"requestId":format!("{group}-{index}"),"locale":keys.locale,"channel":channel_id(h),"inference":v.get("inference"),
                "scenario":{"group":group,"title":input.title,"profile":profile,"instruction":input.instruction,"channel":channel_id(h),"inference":v.get("inference"),"index":index,"comparisonDigest":comparison,"currency":currency.code,"scale":currency.scale,"inputDigest":digest,"members":profiles.len(),"stepLimit":8,"source":source}});
            runs.push(enqueue(a, h, request, None).await?);
        }
        return Ok(json!({"id":group,"runs":runs,"private":true,"synthetic":true}));
    }
    let actor = header(h, "x-rac-user").ok_or(bad("Scenario actor required"))?;
    let rows = scenario_runs(
        &a.db,
        &t,
        actor,
        input.id.as_deref(),
        h.principal.agent_key.as_deref(),
    )
    .await?;
    if name == "merchant.scenario.list" {
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();
        for row in rows {
            let scopes = &row.run_scopes;
            if scopes
                .as_array()
                .is_none_or(|ss| ss.iter().any(|s| s.as_str().is_none_or(|s| !allowed(h, s))))
            {
                continue;
            }
            let r = &row.run_request;
            let s = &r["scenario"];
            if seen.insert(s["group"].as_str().unwrap_or("").to_owned()) {
                items.push(json!({"id":s["group"],"title":s["title"],"members":s["members"],"updatedAt":row.updated_text}));
            }
        }
        let settings = catalogue::settings(a, h, &t).await?;
        let currency = settings.currencies.selected("")?;
        let sample_locale = settings
            .locales
            .iter()
            .find(|l| *l == &settings.main_locale)
            .or_else(|| settings.locales.first())
            .ok_or(bad(
                "Enable a channel language before preparing test customers",
            ))?;
        let catalogue = catalogue::sample(
            host,
            &t,
            channel_id(h),
            sample_locale,
            &currency.code,
            currency.scale,
        )
        .await?;
        let attack = attack_store::latest(host, h, &t).await?;
        return Ok(
            json!({"scenarios":items,"attack":if attack.is_null() {Value::Null} else {json!({"summary":attack["summary"],"total":attack["probes"].as_array().map_or(0, Vec::len),"savedAt":attack["savedAt"],"configurationReview":attack["configurationReview"]})},"configuration":{"channel":channel_id(h),"mainLocale":settings.main_locale,"locales":settings.locales,"countries":settings.countries,"currency":currency.code,"scale":currency.scale,"catalogue":catalogue}}),
        );
    }
    if rows.is_empty() {
        return Err(Error(StatusCode::NOT_FOUND, "Scenario not found".into()));
    }
    let mut runs = Vec::new();
    for row in rows {
        runs.push(Box::pin(operate(a, h, "merchant.run.status", &json!({"id":row.id}))).await?);
    }
    // Running polls stay cheap. Completed evidence uses the release projection.
    if runs
        .iter()
        .all(|r| !matches!(r["state"].as_str(), Some("queued" | "running")))
    {
        let current = evidence::capture(host, &t).await;
        for run in &mut runs {
            run["configurationReview"] = json!(evidence::review(
                &run["preview"]["scenario"]["configurationEvidence"],
                current.as_deref(),
            ));
        }
    }
    Ok(json!({"id":input.id,"runs":runs,"synthetic":true,"execution":"native-commerce-api"}))
}
/// Whether a request or profile ID is a plain identifier.
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
