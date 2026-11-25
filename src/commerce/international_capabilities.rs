//! International configuration and translation MCP tools call the exact same scoped native handlers as HTTP.
use super::{get_scope, merchant_config, method_dependencies, save_config, save_scope};
use crate::foundation::App;
use crate::{auth, translations};
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use veyndo_core::Result;
use veyndo_core::bad;
use veyndo_core::document::{lenient, view};
use veyndo_core::request_context::RequestContext;
pub(crate) fn international_permission(name: &str) -> Option<&'static str> {
    match name {
        "merchant.commerce.read" | "merchant.commerce.dependencies" => Some("settings.read"),
        "merchant.commerce.save" => Some("settings.write"),
        "merchant.translations.list" | "merchant.translations.detail" => Some("catalog.read"),
        "merchant.translations.create"
        | "merchant.translations.control"
        | "merchant.translations.apply" => Some("catalog.write"),
        _ => None,
    }
}
/// The arguments an international commerce capability takes from its call.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct InternationalArgs {
    #[serde(deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    channel_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    area: Option<String>,
    #[serde(deserialize_with = "lenient")]
    method_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    cursor: Option<String>,
    action: Value,
    product_id: Value,
    #[serde(deserialize_with = "lenient")]
    after_product_id: Option<String>,
}

pub(crate) async fn international_invoke(
    a: &App,
    h: &RequestContext,
    name: &str,
    v: &Value,
) -> Result<Value> {
    auth::permit(
        h,
        international_permission(name).ok_or(bad("Unknown international capability"))?,
    )?;
    let state = State(a.clone());
    let headers = h.clone();
    let args: InternationalArgs = view(v);
    let id = || args.id.clone().ok_or(bad("Translation ID required"));
    let Json(result) = match name {
        "merchant.commerce.read" => {
            if let Some(channel) = args.channel_id.as_deref() {
                get_scope(
                    State(axum::extract::FromRef::from_ref(a)),
                    headers,
                    Path(channel.into()),
                )
                .await?
            } else {
                merchant_config(State(axum::extract::FromRef::from_ref(a)), headers).await?
            }
        }
        "merchant.commerce.dependencies" => {
            method_dependencies(
                state,
                headers,
                Path((
                    args.area.clone().ok_or(bad("Method area required"))?,
                    args.method_id.clone().ok_or(bad("Method ID required"))?,
                )),
            )
            .await?
        }
        "merchant.commerce.save" => {
            if let Some(channel) = args.channel_id.as_deref() {
                save_scope(
                    State(axum::extract::FromRef::from_ref(a)),
                    headers,
                    Path(channel.into()),
                    Json(v.clone()),
                )
                .await?
            } else {
                save_config(
                    State(axum::extract::FromRef::from_ref(a)),
                    headers,
                    Json(v.clone()),
                )
                .await?
            }
        }
        "merchant.translations.list" => translations::list(State(a.core.clone()), headers).await?,
        "merchant.translations.create" => {
            translations::create(state, headers, Json(v.clone())).await?
        }
        "merchant.translations.detail" => {
            translations::detail(
                State(a.core.clone()),
                headers,
                Path(id()?),
                axum::extract::Query(HashMap::from([(
                    "cursor".into(),
                    args.cursor.clone().unwrap_or_default(),
                )])),
            )
            .await?
        }
        "merchant.translations.control" => {
            translations::control(
                State(a.core.clone()),
                headers,
                Path(id()?),
                Json(json!({"action":args.action})),
            )
            .await?
        }
        "merchant.translations.apply" => {
            translations::apply(
                state,
                headers,
                Path(id()?),
                Json(json!({"productId":args.product_id})),
            )
            .await?
        }
        _ => return Err(bad("Unknown international capability")),
    };
    if name == "merchant.commerce.read" && args.area.as_deref() == Some("aiPolicy") {
        return Ok(price_policy_view(&result, args.after_product_id.as_deref()));
    }
    Ok(result)
}
/// The JSON fields `price_policy_view` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PricePolicyViewInput {
    #[serde(deserialize_with = "lenient")]
    data: PolicyData,
    revision: Value,
}

/// The settings parts the price policy view projects.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PolicyData {
    #[serde(deserialize_with = "lenient")]
    ai_policy: AiPolicy,
    #[serde(deserialize_with = "lenient")]
    currencies: PolicyCurrencies,
}

/// The AI price policy: autonomy passes through, corridors are paged.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AiPolicy {
    autonomy: Value,
    #[serde(deserialize_with = "lenient")]
    corridors: Vec<Value>,
}

/// The currency settings the view needs; values compare exactly as stored.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PolicyCurrencies {
    pricing_currency: Value,
    #[serde(deserialize_with = "lenient")]
    definitions: Vec<Value>,
}

/// The fields of one corridor the view sorts, pages and matches by.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Corridor {
    product_id: Value,
    currency: Value,
}

/// The fields of one currency definition the view keeps.
#[derive(Deserialize, Default, Serialize)]
#[serde(default)]
struct Definition {
    code: Value,
    scale: Value,
}

/// Small native projection for the existing MCP/planner read, preserving exact amounts and pagination.
fn price_policy_view(snapshot: &Value, after: Option<&str>) -> Value {
    let input: PricePolicyViewInput = view(snapshot);
    let PolicyData {
        ai_policy,
        currencies,
    } = input.data;
    let mut rows = ai_policy
        .corridors
        .into_iter()
        .map(|row| (view::<Corridor>(&row), row))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.0.product_id.as_str().cmp(&b.0.product_id.as_str()));
    let total = rows.len();
    let remaining = rows
        .iter()
        .filter(|(r, _)| {
            after.is_none_or(|cursor| r.product_id.as_str().is_some_and(|id| id > cursor))
        })
        .collect::<Vec<_>>();
    let page = remaining.iter().take(8).copied().collect::<Vec<_>>();
    let definitions = currencies
        .definitions
        .iter()
        .map(view::<Definition>)
        .filter(|c| {
            page.iter().any(|(r, _)| r.currency == c.code) || c.code == currencies.pricing_currency
        })
        .collect::<Vec<_>>();
    let next = if remaining.len() > 8 {
        page.last()
            .map(|(r, _)| r.product_id.clone())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let corridors = page.iter().map(|(_, row)| row).collect::<Vec<_>>();
    json!({"revision":input.revision,"data":{"aiPolicy":{"autonomy":ai_policy.autonomy,"corridors":corridors},"currencies":{"pricingCurrency":currencies.pricing_currency,"definitions":definitions}},"totalCorridors":total,"nextAfterProductId":next})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai_policy_read_projection_is_paged_exact_and_omits_unrelated_settings() {
        let rows = (0..19).rev().map(|i| json!({"productId":format!("product-{i:03}"),"currency":"KWD","minimumMinor":12345,"maximumMinor":23456,"minimumMarginBps":1234})).collect::<Vec<_>>();
        let snapshot = json!({"revision":7,"data":{"unrelated":"private setting","aiPolicy":{"autonomy":{"enabled":false,"maxChangeBps":500,"productsPerDay":20},"corridors":rows},"currencies":{"pricingCurrency":"EUR","definitions":[{"code":"EUR","scale":2,"name":"Euro"},{"code":"KWD","scale":3},{"code":"JPY","scale":0}]}}});
        let first = price_policy_view(&snapshot, None);
        assert_eq!(first["revision"], 7);
        assert_eq!(first["totalCorridors"], 19);
        assert_eq!(
            first["data"]["aiPolicy"]["corridors"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(first["nextAfterProductId"], "product-007");
        assert_eq!(
            first["data"]["aiPolicy"]["corridors"][0]["minimumMinor"],
            12345
        );
        assert_eq!(
            first["data"]["currencies"]["definitions"],
            json!([{"code":"EUR","scale":2},{"code":"KWD","scale":3}])
        );
        assert!(first["data"].get("unrelated").is_none());
        assert!(first.to_string().len() < 6000);
        let second = price_policy_view(&snapshot, first["nextAfterProductId"].as_str());
        assert_eq!(
            second["data"]["aiPolicy"]["corridors"][0]["productId"],
            "product-008"
        );
        let last = price_policy_view(&snapshot, second["nextAfterProductId"].as_str());
        assert_eq!(
            last["data"]["aiPolicy"]["corridors"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert!(last["nextAfterProductId"].is_null());
    }
}
