//! Consolidated dashboard facts in one bounded read; turnover stays separated by invoice currency and is never repriced.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::{Result, State};

/// The overview document the SQL returns; the remaining facts pass through unchanged.
#[derive(Deserialize)]
struct Facts {
    #[serde(default)]
    revenue: Vec<Value>,
    #[serde(default)]
    summary: Map<String, Value>,
    timeline: Vec<Map<String, Value>>,
    variants: Vec<Map<String, Value>>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// One invoice currency's turnover.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Amount {
    #[serde(deserialize_with = "lenient")]
    amount: Option<String>,
    currency: Value,
}

/// One timeline day before its revenue is resolved.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Day {
    #[serde(deserialize_with = "lenient")]
    currency_count: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    revenue: Option<String>,
}

/// An experiment variant's counters.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Variant {
    #[serde(deserialize_with = "lenient")]
    views: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    purchases: Option<i64>,
}

/// One total only when every invoice shares a currency; historical values keep their own.
fn totals(amounts: &[Value]) -> (Option<f64>, Option<Value>) {
    let single = amounts
        .first()
        .filter(|_| amounts.len() == 1)
        .map(view::<Amount>);
    let total = if amounts.is_empty() {
        Some(0.)
    } else {
        single
            .as_ref()
            .and_then(|v| v.amount.as_deref())
            .and_then(|s| s.parse::<f64>().ok())
    };
    (total, single.map(|v| v.currency))
}

/// The overview facts of a shop: summary, timeline and experiment variants.
pub async fn load(a: &State, tenant: &str) -> Result<Value> {
    let facts = veyndo_intelligence::store::overview::facts(a, tenant).await?;
    let mut facts: Facts = serde_json::from_value(facts).expect("SQL overview document");
    let (total, currency) = totals(&facts.revenue);
    facts.summary.insert("revenue".into(), json!(total));
    facts
        .summary
        .insert("revenueByCurrency".into(), json!(facts.revenue));
    facts
        .summary
        .insert("revenueCurrency".into(), json!(currency));
    for day in &mut facts.timeline {
        let d: Day = view(&Value::Object(day.clone()));
        let revenue = if d.currency_count.unwrap_or(0) > 1 {
            Value::Null
        } else {
            json!(
                d.revenue
                    .as_deref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.)
            )
        };
        day.insert("revenue".into(), revenue);
        day.remove("currency_count");
    }
    for variant in &mut facts.variants {
        let v: Variant = view(&Value::Object(variant.clone()));
        let (views, purchases) = (v.views.unwrap_or(0), v.purchases.unwrap_or(0));
        variant.insert(
            "estimate".into(),
            json!((purchases + 1) as f64 / (views + 2) as f64),
        );
    }
    let mut document = facts.rest;
    document.insert("revenue".into(), Value::Null);
    document.insert("summary".into(), Value::Object(facts.summary));
    document.insert("timeline".into(), json!(facts.timeline));
    document.insert("variants".into(), json!(facts.variants));
    Ok(Value::Object(document))
}
