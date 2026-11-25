//! Consolidated dashboard facts in one bounded read; turnover stays separated by invoice currency and is never repriced.
use serde_json::{Value, json};
use veyndo_core::{Result, State};

/// One total only when every invoice shares a currency; historical values keep their own.
fn totals(amounts: Value) -> Value {
    let amounts = amounts.as_array().expect("SQL revenue array");
    let single = amounts.first().filter(|_| amounts.len() == 1);
    json!({"amounts":amounts,"singleTotal":if amounts.is_empty(){Some(0.)}else{single.and_then(|v|v["amount"].as_str()).and_then(|s|s.parse::<f64>().ok())},"singleCurrency":single.map(|v|v["currency"].clone())})
}

/// The overview facts of a shop: summary, timeline and experiment variants.
pub async fn load(a: &State, tenant: &str) -> Result<Value> {
    let mut facts: Value = sqlx::query_file_scalar!("src/store/overview.sql", tenant)
        .fetch_one(&a.db)
        .await?;
    let amounts = facts["revenue"].take();
    let revenue = totals(amounts);
    facts["summary"]["revenue"] = revenue["singleTotal"].clone();
    facts["summary"]["revenueByCurrency"] = revenue["amounts"].clone();
    facts["summary"]["revenueCurrency"] = revenue["singleCurrency"].clone();
    for day in facts["timeline"]
        .as_array_mut()
        .expect("SQL timeline array")
    {
        day["revenue"] = if day["currency_count"].as_i64().unwrap_or(0) > 1 {
            Value::Null
        } else {
            json!(
                day["revenue"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.)
            )
        };
        day.as_object_mut().unwrap().remove("currency_count");
    }
    for variant in facts["variants"]
        .as_array_mut()
        .expect("SQL variants array")
    {
        let views = variant["views"].as_i64().unwrap_or(0);
        let purchases = variant["purchases"].as_i64().unwrap_or(0);
        variant["estimate"] = json!((purchases + 1) as f64 / (views + 2) as f64);
    }
    Ok(facts)
}
