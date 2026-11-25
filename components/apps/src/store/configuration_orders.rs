//! Paginated app-specific immutable purchase projections; filtering happens before LIMIT and never exposes buyer PII.
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{Raw, lenient, view};
use veyndo_core::{Result, State, bad};

/// The JSON fields `configurations` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ConfigurationsInput {
    #[serde(deserialize_with = "lenient")]
    limit: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    after: Option<String>,
}

/// A configuration on an order line, kept as stored, with the app it belongs to.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Configured {
    app: Value,
}

/// The order line fields a configuration page reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Line {
    configuration: Raw<Configured>,
    #[serde(deserialize_with = "lenient")]
    app_configurations: Option<Vec<Raw<Configured>>>,
    referenced_id: Value,
    label: Value,
    quantity: Value,
}

/// One page of the orders that carry a configuration of this app, newest first.
pub async fn configurations(a: &State, t: &str, id: &str, input: &Value) -> Result<Value> {
    let args: ConfigurationsInput = view(input);
    let limit = args.limit.unwrap_or(100);
    let after = args.after.as_deref().unwrap_or("");
    if !(1..=100).contains(&limit) || after.len() > 100 {
        return Err(bad("Invalid configuration order page"));
    }
    let legacy = json!({"cart":{"lineItems":[{"configuration":{"app":id}}]}});
    let multiple = json!({"cart":{"lineItems":[{"appConfigurations":[{"app":id}]}]}});
    let rows = sqlx::query!(
        "SELECT id,data->>'orderNumber' AS number,data->'cart'->'lineItems' AS items FROM orders WHERE tenant=$1 AND (data @> $2::jsonb OR data @> $3::jsonb) AND ($4::text='' OR (created_at,id)<(SELECT created_at,id FROM orders WHERE tenant=$1 AND id=$4)) ORDER BY created_at DESC,id DESC LIMIT $5",
        t,
        legacy,
        multiple,
        after,
        limit + 1
    )
    .fetch_all(&a.db)
    .await?;
    let has_more = rows.len() > limit as usize;
    let mut orders = vec![];
    let mut last = None;
    for row in rows.iter().take(limit as usize) {
        let items: Value = row.items.clone().unwrap_or_default();
        let items: Vec<Line> = items
            .as_array()
            .ok_or(bad("Invalid order configuration snapshot"))?
            .iter()
            .map(view)
            .collect();
        let items = items.iter().filter(|item| item.configuration.app == id || item.app_configurations.as_ref().is_some_and(|v| v.iter().any(|c| c.app == id))).map(|item| {
            // Return only this app's configuration even when multiple apps share a position.
            let configurations = item.app_configurations.as_ref().map(|v| v.iter().filter(|c| c.app == id).map(|c| c.raw.clone()).collect::<Vec<_>>()).unwrap_or_else(|| vec![item.configuration.raw.clone()]);
            json!({"referencedId":item.referenced_id,"label":item.label,"quantity":item.quantity,"configuration":configurations.first(),"appConfigurations":configurations})
        }).collect::<Vec<_>>();
        orders.push(json!({"orderId":row.id,"orderNumber":row.number,"items":items}));
        last = Some(row.id.as_str());
    }
    let next = if has_more { last } else { None };
    Ok(json!({"elements":orders,"hasMore":has_more,"nextCursor":next,"limit":limit}))
}
