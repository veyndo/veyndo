//! One bounded report definition for HTTP, MCP and private workspace presentation preferences.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use veyndo_core::{Result, bad};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Query {
    pub period: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub interval: String,
    pub timezone: String,
    pub channel: Option<String>,
    pub channel_type: String,
    pub currency: Option<String>,
    pub compare: bool,
}
impl Default for Query {
    fn default() -> Self {
        Self {
            period: "last30".into(),
            from: None,
            to: None,
            interval: "auto".into(),
            timezone: "UTC".into(),
            channel: None,
            channel_type: "all".into(),
            currency: None,
            compare: true,
        }
    }
}
impl Query {
    pub fn validate(&self) -> Result<()> {
        if ![
            "today",
            "yesterday",
            "week",
            "last7",
            "last30",
            "last90",
            "month",
            "lastMonth",
            "year",
            "lastYear",
            "all",
            "custom",
        ]
        .contains(&self.period.as_str())
            || !["auto", "day", "week", "month", "year"].contains(&self.interval.as_str())
            || !["all", "storefront", "headless", "marketplace", "pos"]
                .contains(&self.channel_type.as_str())
            || self.timezone.len() > 64
            || self.timezone.parse::<chrono_tz::Tz>().is_err()
            || self
                .channel
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 160)
            || self
                .currency
                .as_ref()
                .is_some_and(|v| v.len() != 3 || !v.bytes().all(|b| b.is_ascii_uppercase()))
            || [&self.from, &self.to].iter().any(|v| {
                v.as_ref().is_some_and(|s| {
                    s.len() != 10 || chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_err()
                })
            })
        {
            return Err(bad("Invalid report filters"));
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Widget {
    id: String,
    kind: String,
    view: String,
    width: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    query: Query,
    widgets: Vec<Widget>,
}
impl Layout {
    pub fn validate(&self) -> Result<()> {
        self.query.validate()?;
        let mut ids = std::collections::HashSet::new();
        if self.widgets.len() > 12
            || self.widgets.iter().any(|w| {
                !veyndo_app_model::declarations::identifier(&w.id)
                    || !ids.insert(&w.id)
                    || ![
                        "revenue",
                        "orders",
                        "average",
                        "refunds",
                        "traffic",
                        "conversion",
                        "products",
                        "channels",
                        "funnel",
                    ]
                    .contains(&w.kind.as_str())
                    || !["line", "bar", "table"].contains(&w.view.as_str())
                    || !["half", "full"].contains(&w.width.as_str())
            })
        {
            return Err(bad("Invalid report layout"));
        }
        Ok(())
    }
}
pub fn schema(n: &str) -> Option<Value> {
    if n != "merchant.reports.read" {
        return None;
    }
    Some(
        json!({"type":"object","additionalProperties":false,"properties":{
        "period":{"type":"string","enum":["today","yesterday","week","last7","last30","last90","month","lastMonth","year","lastYear","all","custom"]},
        "from":{"type":["string","null"],"maxLength":10},"to":{"type":["string","null"],"maxLength":10},
        "interval":{"type":"string","enum":["auto","day","week","month","year"]},"timezone":{"type":"string","maxLength":64},
        "channel":{"type":["string","null"],"maxLength":160},"channelType":{"type":"string","enum":["all","storefront","headless","marketplace","pos"]},
        "currency":{"type":["string","null"],"maxLength":3},"compare":{"type":"boolean"}}}),
    )
}

#[cfg(test)]
mod tests {
    use super::{Layout, Query, schema};
    use serde_json::{Value, json};

    fn query(v: Value) -> Query {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn report_filters_default_to_thirty_days_and_accept_only_known_values() {
        let default = query(json!({}));
        assert_eq!(
            (
                default.period.as_str(),
                default.interval.as_str(),
                default.compare
            ),
            ("last30", "auto", true)
        );
        assert!(default.validate().is_ok());
        assert!(query(json!({"period":"custom","from":"2026-01-01","to":"2026-01-31","interval":"week","timezone":"Europe/Berlin","channel":"web","channelType":"pos","currency":"EUR","compare":false})).validate().is_ok());
        for invalid in [
            json!({"period":"decade"}),
            json!({"interval":"hour"}),
            json!({"channelType":"social"}),
            json!({"timezone":"Mars/Olympus"}),
            json!({"channel":""}),
            json!({"channel":"x".repeat(161)}),
            json!({"currency":"eur"}),
            json!({"currency":"EURO"}),
            json!({"from":"2026-02-30"}),
            json!({"to":"01.01.2026"}),
        ] {
            assert!(query(invalid.clone()).validate().is_err(), "{invalid}");
        }
        assert!(serde_json::from_value::<Query>(json!({"sql":"x"})).is_err());
    }

    #[test]
    fn a_layout_has_at_most_twelve_distinct_known_widgets() {
        let layout = |widgets: Value| {
            serde_json::from_value::<Layout>(json!({"query":{},"widgets":widgets})).unwrap()
        };
        let widget = |id: &str| json!({"id":id,"kind":"revenue","view":"line","width":"half"});
        assert!(layout(json!([])).validate().is_ok());
        assert!(
            layout(json!([widget("a"), {"id":"b","kind":"funnel","view":"table","width":"full"}]))
                .validate()
                .is_ok()
        );
        assert!(
            layout(json!(
                (0..12)
                    .map(|i| widget(&format!("w{i}")))
                    .collect::<Vec<_>>()
            ))
            .validate()
            .is_ok()
        );
        assert!(
            layout(json!(
                (0..13)
                    .map(|i| widget(&format!("w{i}")))
                    .collect::<Vec<_>>()
            ))
            .validate()
            .is_err()
        );
        assert!(
            layout(json!([widget("a"), widget("a")]))
                .validate()
                .is_err()
        );
        for (key, value) in [
            ("id", "Not valid"),
            ("kind", "profit"),
            ("view", "pie"),
            ("width", "third"),
        ] {
            let mut changed = widget("a");
            changed[key] = json!(value);
            assert!(layout(json!([changed])).validate().is_err(), "{key}");
        }
        let invalid_query: Layout =
            serde_json::from_value(json!({"query":{"period":"decade"},"widgets":[]})).unwrap();
        assert!(invalid_query.validate().is_err());
    }

    #[test]
    fn the_report_tool_schema_lists_exactly_the_query_fields() {
        let s = schema("merchant.reports.read").unwrap();
        assert_eq!(s["additionalProperties"], false);
        let fields = json!(Query::default());
        assert_eq!(
            s["properties"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            fields.as_object().unwrap().keys().collect::<Vec<_>>()
        );
        assert!(schema("merchant.reports.write").is_none());
    }
}
