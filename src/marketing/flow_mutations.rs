//! Local flow mutations journal the effect in the same transaction; customer authority changes revoke existing sessions.
use super::flows;
use crate::{App, Result, Value};
pub(crate) async fn execute(
    a: &App,
    t: &str,
    f: &flows::Flow,
    key: &str,
    action: &str,
    config: &Value,
    id: &str,
) -> Result<Value> {
    veyndo_marketing::store::flow_mutations::execute(
        a,
        t,
        veyndo_marketing::store::flow_mutations::Step {
            actor: f.actor.as_deref(),
            locale: &f.locale,
            key,
            action,
        },
        config,
        id,
        &crate::marketing::validate_condition,
    )
    .await
}
