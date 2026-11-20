//! Indexed channel report and a derived semantic graph; observations never imply causation or model learning.
use super::*;
use veyndo_intelligence::store::behavior as store;
pub(super) async fn read(a: &App, h: &RequestContext, t: &str, v: &Value) -> Result<Value> {
    apps::validate_input(&super::schema("knowledge.behavior").unwrap(), v)?;
    let requested_days = v["days"].as_i64().unwrap_or(7) as i32;
    let channel = v["salesChannelId"].as_str().unwrap_or("default");
    let (settings, _) = commerce::scoped_config(a, t, channel).await?;
    let policy = &settings.legal.behavior_analytics;
    let days = requested_days.min(policy.retention_days as i32);
    let version = if policy.enabled && settings.legal.purposes.get("analytics") == Some(&true) {
        legal::policy_version(&settings.legal)
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
            pii: auth::allowed(h, "customers.pii"),
            geo_configured: geography::configured(),
        },
    )
    .await
}
