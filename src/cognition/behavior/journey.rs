//! Private journey drilldown uses the same current receipt, tenant predicates and bounded event projection.
use super::*;
use veyndo_intelligence::store::behavior as store;
pub(super) async fn read(a: &App, h: &RequestContext, t: &str, v: &Value) -> Result<Value> {
    apps::validate_input(&super::schema("knowledge.behavior.journey").unwrap(), v)?;
    let session = v["session"].as_str().ok_or(bad("Session required"))?;
    let channel = store::journey_channel(a, t, session).await?;
    let (s, _) = commerce::scoped_config(a, t, &channel).await?;
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
            version: legal::policy_version(&s.legal),
            customers: auth::allowed(h, "customers.read"),
            orders: auth::allowed(h, "orders.read"),
        },
    )
    .await
}
