//! Each fictional customer has an independent durable provider journal and an authoritative native journey.
use super::super::agent_runs::CURRENT;
use super::super::models::choice;
use super::super::run_journal as checkpoint;
use super::admit;
use super::{evidence, shopper, source};
use crate::ports::AgentHost;
use crate::store::agent_runs::{Lease, complete_scenario, step_pending};
use crate::store::scenario_reports::extension_digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use veyndo_checkout::store::cart_load::load as load_cart;
use veyndo_cognition::inference::Output;
use veyndo_core::RequestContext;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::conflict;
use veyndo_core::document::{lenient, merge, view};
use veyndo_core::hash;
use veyndo_core::model_contract::validate_native;
use veyndo_shop::service::read_cache::settings as scoped_config;
const PROMPT: &str =
    include_str!("../../../../../crates/cognition/prompts/customer-scenario.v1.txt");
fn action_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["action","productId","quantity","query","reason"],"properties":{
        "action":{"type":"string","enum":["search","detail","add","checkout","finish"]},"productId":{"type":"string","maxLength":160},
        "quantity":{"type":"integer","minimum":1,"maximum":10},"query":{"type":"string","maxLength":200},"reason":{"type":"string","maxLength":800}}})
}
/// The durable state of one customer journey, saved after every step in this exact camelCase form.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct JourneyState {
    token: Value,
    products: Vec<Value>,
    configuration_digest: Value,
    snapshot_digest: Value,
    steps: Vec<Value>,
    outcome: String,
    order_id: Value,
    total_minor: Value,
}

impl JourneyState {
    fn load(saved: Value) -> Result<Self> {
        serde_json::from_value(saved).map_err(|_| bad("Invalid scenario journal"))
    }
    fn knows(&self, product: &str) -> bool {
        self.products
            .iter()
            .any(|p| p.get("id").and_then(Value::as_str) == Some(product))
    }
}

/// One shopper action chosen by the model; a saved action is read as it was stored.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ShopperAction {
    #[serde(deserialize_with = "lenient")]
    action: Option<String>,
    #[serde(deserialize_with = "lenient")]
    product_id: Option<String>,
    quantity: Value,
    #[serde(deserialize_with = "lenient")]
    query: Option<String>,
    reason: Value,
}

/// Records what a shopper action led to in the step and the journey; true once the journey is over.
fn record(
    state: &mut JourneyState,
    step: &mut Value,
    action: &ShopperAction,
    result: Result<Value>,
) -> Result<bool> {
    Ok(match result {
        Ok(value) => {
            merge(step, json!({ "observation": value.clone() }))?;
            match action.action.as_deref() {
                Some("search") => {
                    for product in value.as_array().into_iter().flatten() {
                        let id = product.get("id");
                        if let Some(old) = state.products.iter_mut().find(|p| {
                            p.get("id").unwrap_or(&Value::Null) == id.unwrap_or(&Value::Null)
                        }) {
                            *old = product.clone();
                        } else {
                            state.products.push(product.clone());
                        }
                    }
                    false
                }
                Some("checkout") => {
                    state.outcome = "purchased".into();
                    state.order_id = value.get("orderId").cloned().unwrap_or_default();
                    state.total_minor = value.get("totalMinor").cloned().unwrap_or_default();
                    true
                }
                Some("finish") => {
                    state.outcome = "stopped_by_customer".into();
                    true
                }
                _ => false,
            }
        }
        Err(error) => {
            merge(
                step,
                json!({ "error": {"status":error.0.as_u16(),"detail":error.1.chars().take(300).collect::<String>()} }),
            )?;
            state.outcome = "blocked".into();
            true
        }
    })
}
pub async fn execute<H: AgentHost>(
    host: &H,
    t: &str,
    id: &str,
    h: &RequestContext,
    request: &Value,
) -> Result<Value> {
    let a = host.models().core;
    let req: ScenarioRequest = view(request);
    admit(a, h).await?;
    source::recheck(a, h, &req.scenario.source).await?;
    let profile = &req.scenario.profile;
    let mut customer = profile.clone();
    let currency = veyndo_pricing::money::Currency::new(
        req.scenario.currency.as_str().unwrap_or(""),
        req.scenario.scale.as_u64().unwrap_or(255) as u8,
    )
    .map_err(bad)?;
    customer
        .as_object_mut()
        .ok_or(bad("Invalid customer profile"))?
        .remove("budgetMinor");
    merge(
        &mut customer,
        json!({ "budgetAmount": veyndo_pricing::money::Money::new(
            view::<Profile>(profile).budget_minor.unwrap_or(0),
            currency,
        )
        .decimal() }),
    )?;
    merge(
        &mut customer,
        json!({ "budgetCurrency": req.scenario.currency.clone() }),
    )?;
    let choice = choice(request)?;
    let mut state = if let Some(saved) = checkpoint::load(host, "scenario-state").await? {
        JourneyState::load(saved)?
    } else {
        let configuration = evidence::capture(host, t).await;
        let guest = shopper::headers(t, request, None)?;
        let cart = shopper::call(
            host,
            &guest,
            "cart.create",
            &json!({"session":format!("scenario-{id}")}),
        )
        .await?;
        let products = shopper::products(host, &guest, "").await?;
        let (settings, revision) =
            scoped_config(a, t, req.channel.as_str().unwrap_or("default")).await?;
        let extensions = extension_digest(&a.db, t).await?;
        let state = JourneyState {
            token: cart.get("token").cloned().unwrap_or_default(),
            snapshot_digest: json!(hash(&json!({"products":products,"settings":settings,"revision":revision,"extensions":extensions}).to_string())),
            products: products.as_array().cloned().ok_or(bad("Invalid observed catalogue"))?,
            configuration_digest: json!(configuration),
            steps: vec![],
            outcome: "step_limit".into(),
            order_id: Value::Null,
            total_minor: Value::Null,
        };
        checkpoint::save(host, "scenario-state", &json!(state)).await?;
        state
    };
    let guest = shopper::headers(t, request, state.token.as_str())?;
    let start = state.steps.len();
    for index in start..8 {
        if state.outcome != "step_limit" {
            break;
        }
        checkpoint::authorize(host).await?;
        admit(a, h).await?;
        source::recheck(a, h, &req.scenario.source).await?;
        let cart = shopper::call(host, &guest, "cart.quote", &json!({})).await?;
        let prompt = json!({"instruction":req.instruction,"profile":customer,"currency":req.scenario.currency,"minorUnitScale":req.scenario.scale,"products":state.products,"steps":state.steps,"cart":shopper::observation(&cart)}).to_string();
        let action_key = format!("scenario-action-{index}");
        let output: Output = if let Some(saved) = checkpoint::load(host, &action_key).await? {
            serde_json::from_value(saved).map_err(|_| bad("Invalid saved shopper action"))?
        } else {
            let output = checkpoint::model(
                host,
                h,
                choice.as_ref(),
                PROMPT,
                &prompt,
                &action_schema(),
                false,
            )
            .await?;
            validate_native(&action_schema(), &output.value)?;
            checkpoint::save(host, &action_key, &json!(output)).await?;
            output
        };
        let action: ShopperAction = view(&output.value);
        checkpoint::authorize(host).await?;
        source::recheck(a, h, &req.scenario.source).await?;
        let run = CURRENT
            .try_with(Clone::clone)
            .map_err(|_| bad("Scenario requires worker admission"))?;
        let lease = Lease {
            tenant: t,
            id,
            lease: &run.lease,
        };
        if !step_pending(a, lease, &format!("scenario-step-{index}")).await? {
            return Err(conflict("Customer simulation stopped"));
        }
        let result = perform(
            host,
            &guest,
            profile,
            &action,
            &state,
            &format!("scenario-{id}-{index}"),
        )
        .await;
        // Do not publish late evidence after cancellation or a permission change.
        checkpoint::authorize(host).await?;
        let mut step = json!({"index":index,"action":action.action,"reason":action.reason,"model":output.model,"provider":output.provider});
        let done = record(&mut state, &mut step, &action, result)?;
        state.steps.push(step);
        checkpoint::save(host, "scenario-state", &json!(state)).await?;
        if done {
            break;
        }
    }
    source::recheck(a, h, &req.scenario.source).await?;
    let finished_configuration = evidence::capture(host, t).await;
    let report = json!({"scenario":{"title":req.scenario.title,"group":req.scenario.group,"profile":profile,"inputDigest":req.scenario.input_digest,"comparisonDigest":req.scenario.comparison_digest,"currency":req.scenario.currency,"scale":req.scenario.scale,"snapshotDigest":state.snapshot_digest,"configurationEvidence":{"schemaVersion":1,"startedDigest":state.configuration_digest,"finishedDigest":finished_configuration},"steps":state.steps,"outcome":state.outcome,"orderId":state.order_id,"totalMinor":state.total_minor,"synthetic":true,"execution":"native-commerce-api","prompt":veyndo_cognition::prompts::evidence(PROMPT)}});
    checkpoint::authorize(host).await?;
    let run = CURRENT
        .try_with(Clone::clone)
        .map_err(|_| bad("Scenario requires worker admission"))?;
    let lease = Lease {
        tenant: t,
        id,
        lease: &run.lease,
    };
    if !complete_scenario(a, lease, &report).await? {
        return Err(conflict("Scenario completion lost its lease"));
    }
    Ok(report)
}
async fn perform<H: AgentHost>(
    host: &H,
    h: &RequestContext,
    profile: &Value,
    action: &ShopperAction,
    state: &JourneyState,
    key: &str,
) -> Result<Value> {
    let a = host.models().core;
    let product = action.product_id.as_deref().unwrap_or("");
    if matches!(action.action.as_deref(), Some("detail" | "add")) && !state.knows(product) {
        return Err(bad(
            "Customer selected a product outside its observed catalogue",
        ));
    }
    match action.action.as_deref() {
        Some("search") => shopper::products(host, h, action.query.as_deref().unwrap_or("")).await,
        Some("detail") => {
            let value =
                shopper::call(host, h, "catalog.detail", &json!({"productId":product})).await?;
            let found = view::<Detail>(&value).product;
            Ok(
                json!({"id":product,"name":found.name,"description":found.description.as_deref().unwrap_or("").chars().take(800).collect::<String>(),"price":found.price}),
            )
        }
        Some("add") => {
            let cart = load_cart(a, h).await?;
            let quote = shopper::call(host, h, "cart.quote", &json!({})).await?;
            let mut items: Vec<Value> = view::<Basket>(&quote)
                .line_items
                .into_iter()
                .flatten()
                .map(|p| view::<BasketLine>(&p))
                .filter(|p| p.id.as_str() != Some(product))
                .map(|p| json!({"id":p.id,"quantity":p.quantity}))
                .collect();
            // Set this product's exact quantity while retaining the rest of the
            // basket. A lost acknowledgement cannot increment it a second time.
            items.push(json!({"id":product,"quantity":action.quantity}));
            let value = shopper::call(
                host,
                h,
                "cart.replace",
                &json!({"revision":cart.revision,"items":items}),
            )
            .await?;
            Ok(shopper::observation(&value))
        }
        Some("checkout") => {
            let order = shopper::purchase(host, h, profile, key).await?;
            let cart = load_cart(a, h).await?;
            let q = cart.data.order.as_ref().unwrap_or(&order);
            Ok(
                json!({"orderId":view::<BasketLine>(&order).id,"totalMinor":view::<PlacedOrder>(q).cart.price.total_minor.as_deref().and_then(|s|s.parse::<i64>().ok()),"simulated":true}),
            )
        }
        Some("finish") => Ok(json!({"stopped":true})),
        _ => Err(bad("Unsupported customer action")),
    }
}

/// The typed view of a stored scenario run request: its channel, instruction and the scenario it plays.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ScenarioRequest {
    channel: Value,
    instruction: Value,
    #[serde(deserialize_with = "lenient")]
    scenario: Scenario,
}

/// The scenario a run plays: source, synthetic profile, currency and the digests that bind its evidence.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Scenario {
    comparison_digest: Value,
    currency: Value,
    group: Value,
    input_digest: Value,
    profile: Value,
    scale: Value,
    source: Value,
    title: Value,
}

#[cfg(test)]
mod tests {
    use super::{JourneyState, ShopperAction, record};
    use serde_json::{Value, json};
    use veyndo_core::{Error, StatusCode, document::view};

    fn fresh() -> JourneyState {
        JourneyState {
            token: json!("tok"),
            products: vec![json!({"id":"p1","name":"Lamp"})],
            configuration_digest: json!("c"),
            snapshot_digest: json!("s"),
            steps: vec![],
            outcome: "step_limit".into(),
            order_id: Value::Null,
            total_minor: Value::Null,
        }
    }

    #[test]
    fn the_checkpoint_keeps_its_stored_form_and_refuses_a_broken_journal() {
        let saved = json!(fresh());
        assert_eq!(
            saved,
            json!({"token":"tok","products":[{"id":"p1","name":"Lamp"}],"configurationDigest":"c","snapshotDigest":"s","steps":[],"outcome":"step_limit","orderId":null,"totalMinor":null})
        );
        assert_eq!(JourneyState::load(saved).unwrap(), fresh());
        assert!(JourneyState::load(json!({"token":"tok","steps":"x"})).is_err());
    }

    #[test]
    fn steps_update_the_catalogue_and_end_the_journey_like_before() {
        let action = |a: Value| -> ShopperAction { view(&a) };
        let mut state = fresh();
        let mut step = json!({});
        let found = json!([{"id":"p1","name":"Lamp 2"},{"id":"p2"},"odd"]);
        assert!(
            !record(
                &mut state,
                &mut step,
                &action(json!({"action":"search"})),
                Ok(found.clone())
            )
            .unwrap()
        );
        assert_eq!(
            state.products,
            vec![
                json!({"id":"p1","name":"Lamp 2"}),
                json!({"id":"p2"}),
                json!("odd")
            ]
        );
        assert_eq!(step["observation"], found);
        assert!(state.knows("p2") && !state.knows("p3"));
        assert!(
            record(
                &mut state,
                &mut step,
                &action(json!({"action":"checkout"})),
                Ok(json!({"orderId":"o1","totalMinor":1200}))
            )
            .unwrap()
        );
        assert_eq!(
            (state.outcome.as_str(), &state.order_id, &state.total_minor),
            ("purchased", &json!("o1"), &json!(1200))
        );
        let mut blocked = fresh();
        assert!(
            record(
                &mut blocked,
                &mut step,
                &action(json!({"action":7})),
                Err(Error(StatusCode::CONFLICT, "stop".into()))
            )
            .unwrap()
        );
        assert_eq!(blocked.outcome, "blocked");
        assert_eq!(step["error"], json!({"status":409,"detail":"stop"}));
    }
}

/// The budget of a simulated customer profile.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Profile {
    #[serde(deserialize_with = "lenient")]
    budget_minor: Option<i64>,
}

/// A product detail read.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Detail {
    #[serde(deserialize_with = "lenient")]
    product: DetailProduct,
}

/// The product fields a simulated customer sees.
#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailProduct {
    name: Value,
    #[serde(deserialize_with = "lenient")]
    description: Option<String>,
    price: Value,
}

/// A cart quote's line items.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Basket {
    #[serde(deserialize_with = "lenient")]
    line_items: Option<Vec<Value>>,
}

/// A quoted line, or a placed order's ID.
#[derive(Deserialize, Default)]
#[serde(default)]
struct BasketLine {
    id: Value,
    quantity: Value,
}

/// The total of a placed order.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedOrder {
    #[serde(deserialize_with = "lenient")]
    cart: PlacedCart,
}

/// The order's cart price.
#[derive(Deserialize, Default)]
#[serde(default)]
struct PlacedCart {
    #[serde(deserialize_with = "lenient")]
    price: PlacedPrice,
}

/// The order's total in minor units, as a decimal string.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PlacedPrice {
    #[serde(deserialize_with = "lenient")]
    total_minor: Option<String>,
}
