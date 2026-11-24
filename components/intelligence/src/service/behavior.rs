//! Merchant reads of native behavior analytics: the indexed channel report and the private journey, under the current receipt policy and tenant predicates. Observations never imply causation or model learning.
use crate::domain::trusted_geography::configured;
use crate::store::behavior as store;
use serde::Deserialize;
use serde_json::{Value, json};
use veyndo_core::document::{lenient, view};
use veyndo_core::model_contract::validate;
use veyndo_core::{Error, RequestContext, Result, State, StatusCode, bad};
use veyndo_identity::{allowed, merchant, permit};
use veyndo_shop::service::language::language_context;
use veyndo_shop::service::read_cache::settings;
use veyndo_shop::types::legal::version;

/// The input schema of a behavior read.
pub fn schema(name: &str) -> Option<Value> {
    let props = match name {
        "knowledge.behavior" => {
            json!({"days":{"type":"integer","minimum":1,"maximum":30},"salesChannelId":{"type":"string"}})
        }
        "knowledge.behavior.journey" => {
            json!({"session":{"type":"string","minLength":64,"maxLength":64}})
        }
        _ => return None,
    };
    Some(json!({"type":"object","properties":props,"additionalProperties":false}))
}
/// Reads the channel report or one private journey.
pub async fn invoke(a: &State, h: &RequestContext, name: &str, v: &Value) -> Result<Value> {
    let t = merchant(h)?;
    match name {
        "knowledge.behavior" => {
            permit(h, "knowledge.read")?;
            report(a, h, &t, v).await
        }
        "knowledge.behavior.journey" => {
            permit(h, "customers.pii")?;
            journey(a, h, &t, v).await
        }
        _ => Err(bad("Unknown behavioral capability")),
    }
}
/// The JSON fields `report` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReportInput {
    #[serde(deserialize_with = "lenient")]
    days: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    sales_channel_id: Option<String>,
}

/// The channel report with its derived graph.
async fn report(a: &State, h: &RequestContext, t: &str, v: &Value) -> Result<Value> {
    let input: ReportInput = view(v);
    validate(&schema("knowledge.behavior").unwrap(), v)?;
    let requested_days = input.days.unwrap_or(7) as i32;
    let channel = input.sales_channel_id.as_deref().unwrap_or("default");
    let (settings, _) = settings(a, t, channel).await?;
    let policy = &settings.legal.behavior_analytics;
    let days = requested_days.min(policy.retention_days as i32);
    let version = if policy.enabled && settings.legal.purposes.get("analytics") == Some(&true) {
        version(&settings.legal)
    } else {
        String::new()
    };
    let (locale, chain) = language_context(a, h).await?;
    store::report(
        a,
        t,
        store::Report {
            channel,
            days,
            version: &version,
            locale: &locale,
            chain: &chain,
            main_locale: &settings.main_locale,
            retention_days: policy.retention_days,
            enabled: policy.enabled,
            events_per_hour: policy.events_per_hour,
            pii: allowed(h, "customers.pii"),
            geo_configured: configured(),
        },
    )
    .await
}
/// The JSON fields `journey` reads, typed as it reads them.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct JourneyInput {
    #[serde(deserialize_with = "lenient")]
    session: Option<String>,
}

/// One private visitor journey; it needs `customers.pii`.
async fn journey(a: &State, h: &RequestContext, t: &str, v: &Value) -> Result<Value> {
    let input: JourneyInput = view(v);
    validate(&schema("knowledge.behavior.journey").unwrap(), v)?;
    let session = input.session.as_deref().ok_or(bad("Session required"))?;
    let channel = store::journey_channel(a, t, session).await?;
    let (s, _) = settings(a, t, &channel).await?;
    if !s.legal.behavior_analytics.enabled || s.legal.purposes.get("analytics") != Some(&true) {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Behavioral journey not found".into(),
        ));
    }
    store::journey(
        a,
        t,
        store::Journey {
            session,
            channel: &channel,
            retention_days: s.legal.behavior_analytics.retention_days as i32,
            version: version(&s.legal),
            customers: allowed(h, "customers.read"),
            orders: allowed(h, "orders.read"),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn behavior_reads_have_bounded_schemas() {
        assert!(validate(&schema("knowledge.behavior").unwrap(), &json!({"days":7})).is_ok());
        assert!(validate(&schema("knowledge.behavior").unwrap(), &json!({"days":31})).is_err());
        assert!(
            validate(
                &schema("knowledge.behavior.journey").unwrap(),
                &json!({"session":"short"})
            )
            .is_err()
        );
        assert!(schema("knowledge.other").is_none());
    }
}
